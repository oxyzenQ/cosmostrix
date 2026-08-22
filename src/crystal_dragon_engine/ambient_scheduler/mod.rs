// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dynamic idle/wake scheduler thread for the ambient phase system.
//!
//! ## Design: NOT a fixed-interval poller
//!
//! The previous archived `adaptive-custom` engine polled every 30 seconds.
//! That wasted CPU on a quiet machine — most polls found no phase boundary
//! to fire. The user explicitly asked for a **dynamic idle/wake** scheduler:
//!
//! > "dynamic clock — it doesn't have to stay awake continuously; idle
//! > when the time is approaching, then a few seconds before, automatically
//! > wake up — so CPU usage doesn't stay high all the time when the user
//! > uses the ambient config"
//!
//! This module implements that contract. The scheduler thread:
//!
//! 1. Computes `time_to_next_phase` (seconds until the next entry's `HH:MM`
//!    boundary, with midnight wrap-around).
//! 2. Sleeps for that duration (capped at 1 hour for reload responsiveness).
//! 3. On wake, fires the new phase via the mpsc channel.
//! 4. Returns to step 1.
//!
//! Between phase boundaries, the thread is parked in `Condvar::wait_timeout`
//! — zero CPU usage, zero wakeups. The OS only schedules it when:
//!
//! - The timeout expires (a phase boundary was reached), OR
//! - The condvar is notified (live-reload path pushed a new schedule).
//!
//! ## Instant switch
//!
//! The user explicitly asked for **instant switch** (no smoothstep blend
//! window). When the thread fires a phase, the entry is sent to the event
//! loop, which calls `Cloud::apply_ambient_entry` to apply the scene
//! immediately. The only visual smoothing comes from the existing
//! `transition_chars` (glyph warm-start) and `transition_rain_style`
//! (pool reset) — those exist for correctness, not for cinematic blending.
//!
//! ## Live reload
//!
//! When the user saves `config.toml`, the live-reload watcher re-parses the
//! file. If any `ambient.*` keys are present, [`reload_schedule`] is called
//! with the new [`AmbientSchedule`]. This function swaps the schedule
//! atomically (mutex) and notifies the condvar — the thread wakes
//! immediately, recomputes `time_to_next_phase`, and adjusts its sleep.
//!
//! If the new schedule's currently-active phase differs from the previously-
//! applied one, the thread fires it on the next loop iteration (no need to
//! wait for a boundary).
//!
//! ## Edge cases
//!
//! - **Empty schedule**: thread detects `entries.is_empty()`, sleeps 60
//!   seconds, then loops (cheap idle poll). On reload with new entries,
//!   condvar wakes it immediately.
//! - **Single entry**: thread sleeps until the entry's boundary, fires,
//!   then sleeps 24 hours (capped to 1 hour, so it polls hourly — but the
//!   phase is already applied, so it no-ops).
//! - **DST spring-forward**: `current_minute_of_day()` returns wall-clock
//!   local time. Entries in the skipped hour (02:00–02:59) are never fired.
//!   Acceptable — user won't notice.
//! - **DST fall-back**: entries in the repeated hour (01:00–01:59) fire
//!   twice. Acceptable — `apply_ambient_entry` is idempotent.
//! - **Midnight wrap**: handled in `AmbientSchedule::seconds_to_next_phase`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use super::ambient::{
    current_minute_of_day, current_second_of_minute, AmbientEntry, AmbientSchedule,
};

/// Handle returned by [`spawn_ambient_scheduler`].
///
/// The event loop holds this handle and:
/// - Polls [`Self::rx`] each frame (non-blocking `try_recv`) for phase events.
/// - Calls [`Self::reload`] when the live-reload watcher sends a new config.
pub struct AmbientSchedulerHandle {
    /// Receiver for phase-fire events. Each `Ok(AmbientEntry)` means "this
    /// phase's boundary was just crossed — apply it now".
    pub rx: std::sync::mpsc::Receiver<AmbientEntry>,
    /// Shared mutable schedule (swapped atomically by `reload`).
    schedule: Arc<Mutex<AmbientSchedule>>,
    /// Condvar used to wake the thread on schedule reload.
    cv: Arc<Condvar>,
    /// Monotonic counter incremented on every `reload` call.
    ///
    /// closes a TOCTOU race where `reload()` swapped the schedule
    /// and notified the condvar during the window between the scheduler
    /// releasing the lock (after its snapshot) and re-acquiring it for
    /// `wait_timeout`. The notify was lost (no thread was waiting yet),
    /// and the scheduler kept sleeping until the next phase boundary
    /// (potentially hours away). The generation counter lets the scheduler
    /// detect a missed notify by comparing the generation seen during the
    /// snapshot against the generation seen after re-acquiring the lock —
    /// if they differ, skip the wait and loop back immediately.
    generation: Arc<AtomicU64>,
}

impl AmbientSchedulerHandle {
    /// Push a new schedule to the scheduler thread.
    ///
    /// Atomically swaps the schedule (mutex) and notifies the condvar — the
    /// thread wakes immediately, recomputes `time_to_next_phase`, and
    /// adjusts its sleep. If the new schedule's currently-active phase
    /// differs from the previously-applied one, the thread fires it on the
    /// next loop iteration (no boundary wait).
    ///
    /// Called from the event loop's live-reload path
    /// (`event_loop.rs::pending_config`).
    pub fn reload(&self, new: AmbientSchedule) {
        if let Ok(mut s) = self.schedule.lock() {
            *s = new;
        }
        // Increment the generation AFTER the swap so the scheduler can
        // detect the change. The ordering must be SeqCst to pair with the
        // scheduler's load-after-lock check.
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.cv.notify_all();
    }
}

/// Spawn the ambient scheduler thread.
///
/// Returns a handle the caller uses to receive phase events and push
/// schedule reloads. The thread runs for the process lifetime (no stop
/// signal — when the event loop exits, the process exits, killing the
/// thread).
///
/// ## Panic safety
///
/// If the mutex is poisoned (scheduler thread panicked), `reload` will
/// silently no-op (the lock fails internally) and the condvar notify still
/// fires (but the dead thread won't wake). This is acceptable — a
/// scheduler panic is a bug worth investigating, but it shouldn't take down
/// the rain loop.
#[must_use]
pub fn spawn_ambient_scheduler(initial: AmbientSchedule) -> AmbientSchedulerHandle {
    // Pillar 3: bounded channel (cap 64) — prevents unbounded queue growth.
    let (tx, rx) = std::sync::mpsc::sync_channel::<AmbientEntry>(64);
    let schedule = Arc::new(Mutex::new(initial));
    let cv = Arc::new(Condvar::new());
    let generation = Arc::new(AtomicU64::new(0));

    let sched_clone = Arc::clone(&schedule);
    let cv_clone = Arc::clone(&cv);
    let gen_clone = Arc::clone(&generation);

    thread::Builder::new()
        .name("ambient-scheduler".to_string())
        .spawn(move || {
            // S4 (internal independent QA): wrap the scheduler loop in
            // catch_unwind for parity with the live-reload watcher thread
            // (live_config.rs lines 83, 148 both use catch_unwind). Without
            // this, a panic in the time-arithmetic or phase computation
            // would kill the scheduler thread silently — the user would see
            // "ambient stopped working" with no error message. With
            // catch_unwind, a panic is caught and the thread exits cleanly
            // via the normal channel-drop path (tx falls out of scope →
            // rx returns Err → event loop detects dead scheduler).
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                scheduler_loop(sched_clone, cv_clone, gen_clone, tx)
            }));
            if result.is_err() {
                // Buffer the diagnostic so it appears on the main screen
                // post-exit, not in the alt-screen rain matrix (AB-10).
                crate::live_config::push_runtime_warning(
                    "[ambient-scheduler] thread panicked — ambient scheduling disabled for this session",
                );
            }
        })
        .expect("spawn ambient scheduler thread");

    AmbientSchedulerHandle {
        rx,
        schedule,
        cv,
        generation,
    }
}

/// The scheduler thread's main loop. Extracted to a free function so it
/// can be unit-tested with synthetic clocks (the test passes a mock
/// `now_min` instead of calling `current_minute_of_day`).
///
/// Production loop:
/// 1. Read schedule (mutex).
/// 2. Find current phase (latest entry <= now). If different from last
///    applied, fire it via `tx.send`.
/// 3. Compute seconds to next phase boundary.
/// 4. `cv.wait_timeout(sleep_secs)`.
/// 5. Loop.
fn scheduler_loop(
    schedule: Arc<Mutex<AmbientSchedule>>,
    cv: Arc<Condvar>,
    generation: Arc<AtomicU64>,
    tx: std::sync::mpsc::SyncSender<AmbientEntry>,
) {
    // track the FULL last-applied entry (hour, minute, scene) instead
    // of just (hour, minute). This fixes a bug where changing the SCENE NAME
    // for an existing time slot (e.g. `ambient.20-20 = evening` → `ambient.20-20
    // = afternoon`) didn't trigger a refire because the time key was unchanged.
    // Now any change to the scene name triggers a refire.
    let mut last_applied: Option<AmbientEntry> = None;

    // track the day-of-year of the last fire. Without this, a single-entry
    // schedule (e.g. `ambient.22-10 = aurora`) would never refire after the
    // initial fire: at 22:10 the next day, `current_phase == last_applied`
    // (both <22:10, aurora>), so the dedup suppresses the legitimate next-day
    // refire. The day-boundary check below fires the entry once per day when
    // the boundary is crossed, even if `entry == last_applied`.
    //
    // Init to -1 so the first loop iteration always treats today as "new day"
    // — but the existing `last_applied != current_entry` check handles the
    // initial fire, so the day-boundary check is a no-op on the first iteration.
    let mut last_fired_yday: i32 = -1;

    loop {
        let now_min = current_minute_of_day();
        let now_sec = current_second_of_minute();

        // Snapshot the schedule under the lock, compute current phase +
        // sleep duration, then release the lock before sending (so `reload`
        // can't deadlock against a blocked `tx.send`).
        //
        // also snapshot the generation counter so we can detect a
        // missed condvar notify later (see the wait block below).
        let (current_entry, sleep_secs, seen_gen) = {
            let Ok(s) = schedule.lock() else {
                // Mutex poisoned — scheduler can't recover. Exit silently.
                return;
            };
            let current = s.current_phase(now_min).cloned();
            let sleep = s.seconds_to_next_phase(now_min, now_sec).unwrap_or(60);
            let gen = generation.load(Ordering::SeqCst);
            (current, sleep, gen)
        };

        // Fire current phase if its identity changed since last fire.
        // compare the full entry (hour, minute, scene) — not just the
        // time key — so that scene-name changes for an existing slot trigger
        // a refire. This handles three cases:
        //   - Initial startup: last_applied is None → fire current phase.
        //   - Boundary crossed: a new entry became current → fire it.
        //   - Reload changed the active phase OR the active phase's scene
        //     name: last_applied != current_entry → fire.
        if let Some(entry) = &current_entry {
            if last_applied.as_ref() != Some(entry) {
                crate::lr_trace!(
                    "ambient-scheduler: firing phase {:02}:{:02} (scene={})",
                    entry.hour,
                    entry.minute,
                    entry.scene
                );
                if tx.try_send(entry.clone()).is_err() {
                    // Receiver dropped (event loop exited). Terminate.
                    return;
                }
                last_applied = Some(entry.clone());
                last_fired_yday = super::ambient::current_yday();
            }
        } else {
            // AB-09: schedule is empty — clear last_applied so that re-adding
            // the SAME entry (e.g. user uncomments after commenting out at
            // the same hour) triggers a fresh fire. Without this, the dedup
            // check above (`last_applied.as_ref() != Some(entry)`) suppresses
            // the legitimate refire because `last_applied` still holds the
            // previously-applied entry. Symptom: ambient toggle at same hour
            // sometimes doesn't apply, scene stuck on config's default
            // (`scene = cinematic`) instead of the ambient entry's scene
            // (e.g. `signal` / `monolith`). Fixing the root cause here means
            // the event_loop's rx path sees a fresh event after the
            // uncomment, without needing a minute-boundary wake or a manual
            // scene change.
            last_applied = None;
        }

        // day-boundary refire. If we're in a new day (yday changed since
        // the last fire) AND the current phase's boundary has been crossed
        // today (entry.minutes_of_day() <= now_min), refire even if
        // `entry == last_applied`. This handles single-entry schedules where
        // the same entry is "current" across multiple days — without this, a
        // user who presses 'x' after 22:10 would never see aurora re-asserted
        // at 22:10 the next day (the dedup above would suppress it).
        //
        // The check fires AT MOST ONCE per day: after firing, we set
        // `last_fired_yday = today_yday`, so subsequent wakes on the same day
        // see `yday == last_fired_yday` and skip. This prevents refire loops
        // when the scheduler's 1-hour cap triggers multiple wakes per day.
        //
        // Multi-entry schedules are unaffected: the existing
        // `last_applied != current_entry` check above already fires on
        // boundary crossings (different entry), and the day-boundary check
        // is a no-op (`yday == last_fired_yday` after the first fire of the
        // day).
        let today_yday = super::ambient::current_yday();
        if today_yday != last_fired_yday {
            if let Some(entry) = &current_entry {
                if entry.minutes_of_day() <= now_min && last_applied.as_ref() == Some(entry) {
                    // Same entry, new day, past today's boundary — refire.
                    // The `last_applied == Some(entry)` guard ensures we only
                    // take this branch when the existing != check above did
                    // NOT fire (i.e. the entry was already applied on a
                    // previous day). If the entry is new (last_applied is
                    // None or different), the != check above already fired it
                    // and we just mark today as "seen".
                    crate::lr_trace!(
                        "ambient-scheduler: day-boundary refire {:02}:{:02} (scene={}, yday={})",
                        entry.hour,
                        entry.minute,
                        entry.scene,
                        today_yday
                    );
                    if tx.try_send(entry.clone()).is_err() {
                        return;
                    }
                }
            }
            // Mark today as "seen" — whether or not we fired. This prevents
            // repeated refire attempts on subsequent wakes within the same
            // day (the 1-hour sleep cap can trigger multiple wakes per day).
            last_fired_yday = today_yday;
        }

        // Sleep until next phase boundary OR reload signal.
        // Cap at 1 hour so a long-running session still picks up reload
        // signals even if the condvar notify is missed (defense-in-depth).
        //
        // race fix: between releasing the lock above (after the
        // snapshot) and re-acquiring it here for `wait_timeout`, a `reload`
        // call can swap the schedule AND notify the condvar. That notify is
        // lost because no thread is waiting yet. Without the generation
        // check, the scheduler would sleep for `sleep_secs` (computed from
        // the OLD schedule, potentially hours) and miss the reload entirely.
        //
        // The fix: after re-acquiring the lock, compare the current
        // generation against `seen_gen`. If they differ, a reload happened
        // during the window — skip the wait and loop back immediately to
        // re-snapshot the new schedule.
        let sleep_dur = Duration::from_secs(sleep_secs.min(3600));
        let _guard = {
            let Ok(s) = schedule.lock() else {
                return;
            };
            let current_gen = generation.load(Ordering::SeqCst);
            if current_gen != seen_gen {
                // Reload happened during the firing window — don't wait.
                // Drop the lock and loop back to re-snapshot.
                drop(s);
                continue;
            }
            // robustness: if the mutex is poisoned (a prior `reload`
            // panicked mid-swap — extremely unlikely but possible),
            // `wait_timeout` returns Err. We treat that the same as a
            // poisoned lock above: exit the scheduler thread silently
            // rather than panicking. A dead scheduler is recoverable on
            // next process restart; a panicking scheduler thread would
            // print a backtrace to stderr mid-rain (flicker in alternate-
            // screen mode). Matches the poison-handling pattern documented
            // in the module-level panic-safety section above.
            let Ok((g, _timeout_result)) = cv.wait_timeout(s, sleep_dur) else {
                return;
            };
            g
        };
        // Loop back: recompute now_min, find current phase, fire if changed.
        // If the wake was a timeout (boundary reached), `current_phase` will
        // return the new entry, `last_applied` won't match, and we fire.
        // If the wake was a condvar notify (reload), the new schedule is in
        // place, and we recompute from it.
    }
}

#[cfg(test)]
mod tests;
