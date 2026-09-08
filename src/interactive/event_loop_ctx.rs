// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-21 (wart #3, owner mandate 2026-09-08): the event
//! loop's context struct.
//!
//! ## Why this exists
//!
//! The v50.0.0-beta.7 LOC refactor split `event_loop.rs` into 17
//! sibling modules, but the loop's mutable state stayed as ~45 locals
//! in `run_interactive`, threaded into the siblings as positional
//! `&mut` parameters. That left two structural hazards:
//!
//! 1. **Same-type cross-wiring.** Several sibling signatures carried
//!    adjacent parameters of identical type — `charset_preset: &mut
//!    String, scene_name: &mut String`, `base_cfg: &mut CloudConfig,
//!    current_cfg: &mut CloudConfig, startup_cfg: &CloudConfig, cfg:
//!    &CloudConfig` (four same-typed config refs in
//!    `apply_config_rebuild`), the `u64`/`f64` accumulator wall in
//!    `update_perf_stats`, `w: u16, h: u16`. Swapping two of those at
//!    a call site compiles cleanly and silently routes state to the
//!    wrong layer. Thirteen `#[allow(clippy::too_many_arguments)]`
//!    suppressions across the family quantified the wart.
//! 2. **Signature churn.** Adding one piece of loop state meant
//!    editing the signatures (and every call site) of every sibling
//!    that needed it — the reason `event_loop.rs` carried a
//!    LOC_EXEMPT ("further splitting requires a context struct
//!    refactor").
//!
//! The fix: bundle the state into [`LoopCtx`] with domain sub-structs
//! so every former positional hazard becomes a *named field*
//! (`ctx.config.base` vs `ctx.config.current` cannot be transposed),
//! and the two monster functions
//! ([`apply_config_rebuild`][super::event_loop_config_rebuild] and
//! [`poll_ambient_events`][super::event_loop_ambient]) take one
//! `&mut LoopCtx` instead of 21-23 parameters.
//!
//! ## Sub-struct grouping
//!
//! - [`SceneIdentity`] — the scene-family trio (`x`/`X` cycle, `s`/`S`
//!   cycle, ambient applies, live-reload rebuilds all mutate it).
//! - [`ConfigLayers`] — the three CloudConfig layers (pristine
//!   startup / runtime-mutable base / live-reloaded current) + the
//!   watcher payload slots. `startup` is never mutated for the whole
//!   session and is content-identical to the `cfg: &CloudConfig`
//!   argument `run_interactive` receives — the old code threaded BOTH
//!   into the siblings; they are now one field.
//! - [`AmbientState`] — the ambient scheduler handle + tracker fields.
//! - [`PerfCounters`] — the post-exit report accumulators.
//!
//! ## Construction
//!
//! Built once in `run_interactive` AFTER the intro sequence (the intro
//! owns term/cloud/frame/w/h before the loop state exists — it keeps
//! its granular signature) and BEFORE the startup-ambient apply (that
//! block mutates ctx fields like every other pre-frame step). The
//! rain loop then reads and writes `ctx.<field>` exclusively.
//!
//! ## Performance
//!
//! Zero: every field was already a stack local (or heap owner) in
//! `run_interactive`; moving it into one struct changes only name
//! resolution (`ctx.cloud` vs `cloud`), which compiles to the same
//! stack-relative addressing. The one-time move-construction is
//! pointer copies. No allocation is added or removed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use super::activity::FrameTimeTracker;
use super::adaptive::{EnduranceHealth, PerformanceSelfHealer, ReclaimState};
use super::event_loop_post_draw::EffectsAutoGate;
use super::hud::HudState;
use super::input::PasteBurstGuard;
use crate::app::CloudConfig;
use crate::central_control_power_dragon::PowerManager;
use crate::cloud::Cloud;
use crate::crystal_dragon_engine::ambient::{AmbientEntry, AmbientSchedule};
use crate::crystal_dragon_engine::ambient_scheduler::AmbientSchedulerHandle;
use crate::frame::Frame;
use crate::terminal::Terminal;

/// Scene-family identity — the trio every scene-change path mutates.
///
/// `scene_generation` is the Phase D change counter (u64 compare
/// instead of String clone): bumped by every scene-family mutation
/// (`x`/`X` cycle, ambient apply, snapback, overlay-lift revert,
/// live-reload rebuild) so the adaptive throttler and the self-healer
/// can detect "scene changed since frame start" with one integer
/// compare. Keeping the three together means helpers take one
/// `&mut SceneIdentity` instead of the old
/// `&mut String, &mut String, &mut u64` positional triple — the
/// two `&mut String`s were the family's most transposable pair.
pub(crate) struct SceneIdentity {
    /// Active charset preset name (`s`/`S` cycle confirmation source).
    pub charset_preset: String,
    /// Active scene name (`x`/`X` cycle confirmation source).
    pub scene_name: String,
    /// Scene-family change counter (see struct docs).
    pub scene_generation: u64,
}

/// The three CloudConfig layers + the live-reload payload slots.
///
/// Startup resolution: `CLI > config.toml > scene defaults` is baked
/// into `startup`. Runtime contract (v80.0.0-beta.1, owner mandate
/// 2026-09-01):
///
/// - `startup` — pristine snapshot, **never mutated** for the whole
///   session. The live-reload path restores the scene family from it
///   when the config `scene` key is removed, so a `--scene
///   crystal-dragon` run returns to crystal-dragon after the config
///   override is commented back out — no exit, no rerun.
///   Content-identical to the `cfg: &CloudConfig` argument
///   `run_interactive` receives (the old code threaded both into
///   siblings; they are one field now).
/// - `base` — runtime-mutable: the scene-family sync (shortkey /
///   ambient preservation) writes managed defaults into it before
///   each rebuild.
/// - `current` — the LATEST live-reloaded effective config;
///   `finalize_session` reads it at exit so the "final runtime state"
///   verbose section shows live values, not startup values.
/// - `pending` — the watcher payload, applied at the top of the next
///   frame by
///   [`apply_config_rebuild`][super::event_loop_config_rebuild].
/// - `last_applied_map` — the last applied config map (diff trace +
///   ambient scene-custom lookups).
pub(crate) struct ConfigLayers {
    /// Pristine startup snapshot (CLI > config > scene resolution).
    pub startup: CloudConfig,
    /// Runtime-mutable base (scene-family sync writes).
    pub base: CloudConfig,
    /// Latest live-reloaded effective config.
    pub current: CloudConfig,
    /// Pending watcher payload (applied next frame).
    pub pending: Option<HashMap<String, String>>,
    /// Last applied config map (diff trace source).
    pub last_applied_map: Option<HashMap<String, String>>,
}

/// Ambient scheduler state — the handle plus the tracker fields the
/// rx-event / snapback / ground-truth guards share.
pub(crate) struct AmbientState {
    /// Scheduler handle (rx channel + reload).
    pub handle: AmbientSchedulerHandle,
    /// Last schedule snapshot (reload detection + staleness guard).
    pub last_schedule: AmbientSchedule,
    /// Last-applied ambient entry (re-applied after rebuilds).
    pub last_applied_entry: Option<AmbientEntry>,
    /// AB-07: once the schedule is detected empty (by any path),
    /// auto-snapback is disabled until a new rx event is applied
    /// from a non-empty schedule.
    pub snapback_killed: bool,
    /// AB-08 ground-truth re-read rate limiter (1 per 5 s).
    pub last_ground_truth_check: Instant,
    /// AB-08: config file path for the ground-truth re-read (the
    /// watcher can lose events; the file on disk is truth).
    pub path_for_ground_truth: Option<PathBuf>,
}

/// Post-exit report accumulators (the `--perf-stats` summary + the
/// final FPS line + SessionStats inputs).
///
/// All twelve were individual `&mut` parameters of
/// `update_perf_stats` — the `f64` trio (`work_sum_s`,
/// `pressure_sum`, `utilization_sum`) and the `u64` quintet were
/// positionally transposable. Named fields end that.
pub(crate) struct PerfCounters {
    /// Total rendered frames (drawn + idle).
    pub frames: u64,
    /// Frames that actually emitted cells.
    pub drawn_frames: u64,
    /// Frames skipped (no dirty cells).
    pub idle_frames: u64,
    /// Frames whose write latency exceeded the frame budget (HUNT-23
    /// drain signal).
    pub overshoot_frames: u64,
    /// Σ work time (seconds) across drawn frames.
    pub work_sum_s: f64,
    /// Max work time (seconds).
    pub work_max_s: f64,
    /// Σ effective pressure.
    pub pressure_sum: f64,
    /// Max effective pressure.
    pub pressure_max: f32,
    /// Σ frame utilization (work / budget).
    pub utilization_sum: f64,
    /// Max frame utilization.
    pub utilization_max: f32,
    /// Σ dirty cells (perceived-motion diagnostics).
    pub dirty_sum: u64,
    /// Dirty-cell sample count.
    pub dirty_samples: u64,
}

/// Per-frame draw observation — the outputs of the sim+draw phase and
/// the post-draw accounting that the frame-tail siblings
/// (`update_perf_stats`, `sample_p5_health`) consume.
///
/// These eight values were the trailing positional parameters of
/// `update_perf_stats` (22 params total) — the three `f32` values
/// (`work_s`, `overshoot`, `utilization`) plus `frame_period_s` were
/// positionally transposable. One named-field struct ends that, and
/// gives the two frame-tail consumers a shared observation object so
/// they can never disagree about what happened in the frame.
pub(crate) struct FrameObs {
    /// Pre-work timestamp (scheduling anchor for the next frame).
    pub work_start: Instant,
    /// Render work time for this frame (seconds).
    pub work_s: f32,
    /// Effective frame period (seconds) — work/budget utilization
    /// denominator.
    pub frame_period_s: f32,
    /// Whether this frame emitted cells (gates the overshoot signal,
    /// HUNT-23: stale `last_write_ns` on non-drawing frames must not
    /// feed the drain backoff).
    pub did_draw: bool,
    /// Whether the frame flagged full repaint (dirty count = full
    /// screen).
    pub is_dirty_all: bool,
    /// Dirty cell count for this frame.
    pub dirty_len: usize,
    /// Write-latency overshoot vs the frame budget (seconds).
    pub overshoot: f32,
    /// Frame utilization (work / budget).
    pub utilization: f32,
}

/// The event loop's mutable session state — one struct, one owner
/// (`run_interactive`), borrowed per-frame by the sibling modules.
///
/// Field groups follow the loop's phase order: render core, pacing +
/// power, adaptive + health, session identity, config + ambient,
/// HUD + input, perf accounting. `user_ranges` and `def_ascii` are
/// immutable after construction but live here so the siblings that
/// need them (rebuild, ambient, keybinding) read them from the same
/// context instead of taking extra parameters.
pub(crate) struct LoopCtx {
    // ── Render core ──
    /// Terminal writer (moved out into `finalize_session` at exit).
    pub term: Terminal,
    /// The rain simulation + render state.
    pub cloud: Cloud,
    /// Differential frame buffer.
    pub frame: Frame,
    /// Current terminal width (resize updates).
    pub w: u16,
    /// Current terminal height (resize updates).
    pub h: u16,

    // ── Frame pacing + power ──
    /// Owns perf_pressure, idle detection, effective FPS, drain
    /// backoff.
    pub power_manager: PowerManager,
    /// Next frame deadline (ideal-cadence anchor).
    pub next_frame: Instant,
    /// Last idle-resync (Cloud concern, not power).
    pub last_resync_time: Instant,

    // ── Adaptive + health ──
    /// P4 madvise rate-limiter.
    pub reclaim_state: ReclaimState,
    /// P5 endurance health score.
    pub endurance_health: EnduranceHealth,
    /// P1+P2 performance self-healer.
    pub self_healer: PerformanceSelfHealer,
    /// S-master-HUNT-24 dynamic effects congestion gate.
    pub effects_auto_gate: EffectsAutoGate,
    /// P5 context-switch sample timestamp (Linux `/proc`; other
    /// platforms keep the field but never read it).
    pub last_ctxt_sample: Instant,
    /// P5 previous context-switch count (Linux only).
    #[cfg(target_os = "linux")]
    pub last_ctxt_switches: u64,
    /// P5 RSS sample count.
    pub perf_rss_samples: u64,

    // ── Session identity ──
    /// Scene-family trio (charset / scene / generation).
    pub scene: SceneIdentity,
    /// Immutable custom glyph ranges (charset resolution input).
    pub user_ranges: Vec<(char, char)>,
    /// Immutable "default ASCII charset" flag.
    pub def_ascii: bool,

    // ── Config + ambient ──
    /// The three config layers + watcher payload slots.
    pub config: ConfigLayers,
    /// Ambient scheduler state.
    pub ambient: AmbientState,

    // ── HUD + input ──
    /// Live HUD overlay ('i' toggles; zero cost when off).
    pub hud_state: HudState,
    /// Bracketed-paste burst guard (v50 kitty-keyboard protocol).
    pub paste_guard: PasteBurstGuard,
    /// Last user input (auto-snapback idle driver).
    pub last_user_input_at: Instant,
    /// Resize debounce timestamp.
    pub last_resize_event: Option<Instant>,

    // ── Perf accounting ──
    /// Post-exit report accumulators.
    pub perf: PerfCounters,
    /// Rolling frame-time tracker (p99 / HUD fps source).
    pub frame_time_tracker: FrameTimeTracker,
}

/// The render core the loop context is seeded from — the five values
/// `setup_terminal_cloud_frame` produced and the intro sequence may
/// have updated (terminal re-measure, bug #10). Bundling them gives
/// [`LoopCtx::new`] a six-argument constructor (under the clippy
/// threshold) and turns the `w`/`h` `u16` pair into named fields.
pub(crate) struct LoopCtxCore {
    /// Terminal writer (moved out into `finalize_session` at exit).
    pub term: Terminal,
    /// The rain simulation + render state.
    pub cloud: Cloud,
    /// Differential frame buffer.
    pub frame: Frame,
    /// Measured terminal width.
    pub w: u16,
    /// Measured terminal height.
    pub h: u16,
}

impl LoopCtx {
    /// Construct the loop context from the post-intro render core +
    /// the startup-resolved config.
    ///
    /// `core` carries the five render-core values the intro sequence
    /// may have updated (terminal, cloud, frame, and the measured
    /// width/height — bundled so the constructor stays under the
    /// clippy argument threshold AND the `w`/`h` `u16` pair becomes
    /// named fields, closing the last positional swap hazard).
    ///
    /// `startup` is moved in as the pristine layer; `base` and
    /// `current` start as its clones (they diverge at the first
    /// runtime scene sync / live reload). The ambient schedule
    /// snapshot and the ground-truth config path are derived from
    /// `startup` — exactly the initializations the old
    /// `run_interactive` locals performed (the ambient handle itself
    /// is spawned by the caller before this call, from the same
    /// schedule). Every other field starts at its neutral default,
    /// mirroring the old local initializations one-for-one.
    pub(crate) fn new(
        core: LoopCtxCore,
        startup: CloudConfig,
        ambient_handle: AmbientSchedulerHandle,
        scene: SceneIdentity,
        user_ranges: Vec<(char, char)>,
        def_ascii: bool,
    ) -> Self {
        let LoopCtxCore {
            term,
            cloud,
            frame,
            w,
            h,
        } = core;
        let ambient_last_schedule = startup.ambient_schedule.clone();
        let ambient_path = startup.config_path_for_watcher.clone();
        Self {
            term,
            cloud,
            frame,
            w,
            h,
            power_manager: PowerManager::new(startup.target_fps, Instant::now()),
            next_frame: Instant::now(),
            last_resync_time: Instant::now(),
            reclaim_state: ReclaimState::new(),
            endurance_health: EnduranceHealth::new(),
            self_healer: PerformanceSelfHealer::new(),
            effects_auto_gate: EffectsAutoGate::new(),
            last_ctxt_sample: Instant::now(),
            #[cfg(target_os = "linux")]
            last_ctxt_switches: 0,
            perf_rss_samples: 0,
            scene,
            user_ranges,
            def_ascii,
            config: ConfigLayers {
                base: startup.clone(),
                current: startup.clone(),
                startup,
                pending: None,
                last_applied_map: None,
            },
            ambient: AmbientState {
                handle: ambient_handle,
                last_schedule: ambient_last_schedule,
                last_applied_entry: None,
                snapback_killed: false,
                last_ground_truth_check: Instant::now(),
                path_for_ground_truth: ambient_path,
            },
            hud_state: HudState::new(),
            paste_guard: PasteBurstGuard::default(),
            last_user_input_at: Instant::now(),
            last_resize_event: None,
            perf: PerfCounters {
                frames: 0,
                drawn_frames: 0,
                idle_frames: 0,
                overshoot_frames: 0,
                work_sum_s: 0.0,
                work_max_s: 0.0,
                pressure_sum: 0.0,
                pressure_max: 0.0,
                utilization_sum: 0.0,
                utilization_max: 0.0,
                dirty_sum: 0,
                dirty_samples: 0,
            },
            frame_time_tracker: FrameTimeTracker::new(),
        }
    }
}
