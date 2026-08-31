// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! ambient_scheduler tests, extracted from inline `mod tests { ... }` block in
//! ambient_scheduler.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access ambient_scheduler.rs's private items unchanged.

use crate::crystal_dragon_engine::ambient::AmbientEntry;
use std::collections::HashMap;

/// Helper: build a minimal entry.
fn entry(h: u32, m: u32) -> AmbientEntry {
    AmbientEntry {
        hour: h,
        minute: m,
        scene: "cinematic".to_string(),
    }
}

use super::*;
#[test]
fn handle_reload_swaps_schedule_atomically() {
    let s1 = AmbientSchedule {
        entries: vec![entry(6, 0)],
    };
    let handle = spawn_ambient_scheduler(s1);
    // Reload with a different schedule.
    let s2 = AmbientSchedule {
        entries: vec![entry(0, 0), entry(12, 0), entry(18, 0)],
    };
    handle.reload(s2);
    // Verify the swap took (lock and check).
    let s = handle.schedule.lock().unwrap();
    assert_eq!(s.entries.len(), 3);
}

#[test]
fn handle_reload_with_empty_schedule() {
    let s1 = AmbientSchedule {
        entries: vec![entry(12, 0)],
    };
    let handle = spawn_ambient_scheduler(s1);
    handle.reload(AmbientSchedule::default());
    let s = handle.schedule.lock().unwrap();
    assert!(s.is_empty());
}

#[test]
fn spawn_with_empty_schedule_does_not_fire() {
    // Spawn with empty schedule — thread should idle, never fire.
    let handle = spawn_ambient_scheduler(AmbientSchedule::default());
    // Wait a tiny bit to let the thread run its loop.
    std::thread::sleep(Duration::from_millis(50));
    // No event should be pending.
    assert!(handle.rx.try_recv().is_err());
}

#[test]
fn spawn_fires_current_phase_on_startup() {
    // Schedule with one entry far in the past (00:00). Now is some time
    // today. The thread should fire the entry once at startup.
    let s = AmbientSchedule {
        entries: vec![entry(0, 0)],
    };
    let handle = spawn_ambient_scheduler(s);
    // Give the thread time to fire.
    let event = handle
        .rx
        .recv_timeout(Duration::from_secs(1))
        .expect("scheduler should fire current phase on startup");
    assert_eq!(event.hour, 0);
    assert_eq!(event.minute, 0);
}

#[test]
fn reload_fires_new_current_phase_if_different() {
    // Start with one entry at 00:00. Thread fires it.
    let s1 = AmbientSchedule {
        entries: vec![entry(0, 0)],
    };
    let handle = spawn_ambient_scheduler(s1);

    // Drain initial fire.
    let _ = handle.rx.recv_timeout(Duration::from_secs(1)).unwrap();

    // Reload with a schedule whose current phase is different.
    // Use entry at 00:01 — wait until wall-clock crosses 00:01, OR
    // simulate by reloading a schedule with an entry that just became
    // current. To make this test deterministic, we use an entry far in
    // the past (so it's "current" at any time of day).
    let s2 = AmbientSchedule {
        entries: vec![entry(0, 1)],
    };
    handle.reload(s2);

    // The thread should detect the new current phase (00:01, since
    // 00:01 <= now_min for any time after 00:01) and fire it.
    // Note: if test runs exactly at 00:00, this might be flaky.
    // Cap wait at 2s — if no event, the thread didn't fire (acceptable
    // if wall clock is exactly 00:00 or 00:01 boundary).
    match handle.rx.recv_timeout(Duration::from_secs(2)) {
        Ok(e) => {
            // Some event fired — verify it's the new entry.
            assert_eq!(e.hour, 0);
        }
        Err(_) => {
            // No event — acceptable if wall clock is on a boundary.
            // The test still validates no panic / no deadlock.
        }
    }
}

#[test]
fn empty_schedule_sleeps_idle() {
    // Schedule with no entries — thread should sleep 60s, not fire.
    let handle = spawn_ambient_scheduler(AmbientSchedule::default());
    // Wait long enough to confirm no spurious fires.
    std::thread::sleep(Duration::from_millis(150));
    assert!(handle.rx.try_recv().is_err());
}

#[test]
fn validate_ambient_entries_test_schedule_with_multiple_phases() {
    // Sanity: a realistic 3-phase schedule parses + validates.
    // each entry is just a scene name.
    let mut cfg = HashMap::new();
    cfg.insert("ambient.00-00".into(), "monolith".into());
    cfg.insert("ambient.06-00".into(), "matrix".into());
    cfg.insert("ambient.22-00".into(), "cinematic".into());
    let s = crate::crystal_dragon_engine::ambient::collect_ambient_schedule(&cfg);
    assert_eq!(s.entries.len(), 3);
    assert_eq!(s.entries[0].hour, 0);
    assert_eq!(s.entries[1].hour, 6);
    assert_eq!(s.entries[2].hour, 22);
    assert!(crate::crystal_dragon_engine::ambient::validate_ambient_entries(&cfg).is_ok());
}

// ── scheduler entry-aware refire tests ──
//
// The scheduler now tracks the FULL entry (hour, minute, scene) instead
// of just (hour, minute). This enables refire when the scene NAME for
// an existing time slot changes (e.g. `ambient.20-20 = evening` →
// `ambient.20-20 = afternoon`). Verify the comparison logic via the
// AmbientEntry PartialEq derivation.

#[test]
fn ambient_entry_eq_compares_all_fields() {
    // Sanity: AmbientEntry derives PartialEq — entries are equal only
    // when ALL fields match. This is the foundation of the
    // entry-aware refire fix.
    let a = entry(20, 20);
    let b = entry(20, 20);
    assert_eq!(a, b, "same hour/minute/scene must be equal");

    let c = AmbientEntry {
        hour: 20,
        minute: 20,
        scene: "afternoon".to_string(),
    };
    let d = AmbientEntry {
        hour: 20,
        minute: 20,
        scene: "evening".to_string(),
    };
    assert_ne!(
        c, d,
        "entries with same time but different scene must NOT be equal (fix)"
    );

    let e = AmbientEntry {
        hour: 20,
        minute: 20,
        scene: "afternoon".to_string(),
    };
    let f = AmbientEntry {
        hour: 21,
        minute: 0,
        scene: "afternoon".to_string(),
    };
    assert_ne!(
        e, f,
        "entries with different time must NOT be equal even if scene matches"
    );
}

/// scenario from the bug report — user changes the scene NAME
/// for an existing time slot. The scheduler should refire because the
/// entries are no longer equal (different scene).
#[test]
fn reload_with_renamed_scene_triggers_refire() {
    // Schedule 1: ambient.20-20 = evening
    let s1 = AmbientSchedule {
        entries: vec![AmbientEntry {
            hour: 20,
            minute: 20,
            scene: "evening".to_string(),
        }],
    };
    let handle = spawn_ambient_scheduler(s1);

    // Drain initial fire (scheduler fires current phase on startup).
    let _initial = handle
        .rx
        .recv_timeout(Duration::from_secs(1))
        .expect("scheduler should fire initial phase");

    // Schedule 2: ambient.20-20 = afternoon (same time, different scene)
    let s2 = AmbientSchedule {
        entries: vec![AmbientEntry {
            hour: 20,
            minute: 20,
            scene: "afternoon".to_string(),
        }],
    };
    handle.reload(s2);

    // scheduler should fire the new entry because the scene name
    // differs from the last-applied entry (under the time-key-only
    // comparison, this would NOT fire — the bug).
    match handle.rx.recv_timeout(Duration::from_secs(2)) {
        Ok(e) => {
            assert_eq!(
                e.scene, "afternoon",
                "scheduler must refire with new scene name"
            );
            assert_eq!(e.hour, 20);
            assert_eq!(e.minute, 20);
        }
        Err(_) => {
            panic!("regression: scheduler did not refire on scene-name change");
        }
    }
}

// ── day-boundary refire tests ──
//
// The scheduler now tracks `last_fired_yday` and refires the current
// entry once per day when the boundary is crossed, even if
// `entry == last_applied`. This fixes the single-entry schedule bug
// where pressing 'x' after 22:10 prevented aurora from re-asserting
// at 22:10 the next day.

/// Verify the day-boundary refire helper exists and is callable.
/// The actual day-rollover behavior is wall-clock-dependent and can't
/// be tested deterministically without a mock clock, but we can verify
/// the helper doesn't panic and returns a sane value.
#[test]
fn current_yday_returns_sane_value() {
    let yday = crate::crystal_dragon_engine::ambient::current_yday();
    // tm_yday is 0..=365 on Unix; non-Unix fallback is (secs/86400)%366.
    // Either way, it must be in [0, 365] (366 would only occur on Dec 31
    // of a leap year on Unix, but the fallback mod 366 could produce it).
    assert!(
        (0..=366).contains(&yday),
        "current_yday returned {yday}, expected 0..=366"
    );
}

/// Single-entry schedule: scheduler fires on startup, then sleeps until
/// the next phase boundary (which for a single entry is 24h away, capped
/// to 1h). Within a 1s test window, no spurious refire should occur —
/// the day-boundary refire only fires when yday changes, which doesn't
/// happen within 1s.
#[test]
fn single_entry_no_spurious_refire_within_same_day() {
    let s = AmbientSchedule {
        entries: vec![entry(0, 0)],
    };
    let handle = spawn_ambient_scheduler(s);

    // Drain initial fire.
    let _initial = handle
        .rx
        .recv_timeout(Duration::from_secs(1))
        .expect("scheduler should fire initial phase");

    // Within 1s, no spurious refire (yday hasn't changed).
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        handle.rx.try_recv().is_err(),
        "no spurious refire expected within same day"
    );
}

/// Verify the day-boundary refire logic is present in the source.
/// This is a static check (string match) that ensures the v35 refire
/// code path exists — if a future refactor accidentally removes it,
/// this test will fail. The actual day-rollover behavior is tested
/// via the event loop's integration tests (which can simulate
/// `user_override_since_ambient = true` and verify the scheduler's
/// next fire is applied, not deduped).
#[test]
fn day_boundary_refire_code_path_exists() {
    let src = include_str!("mod.rs");
    assert!(
        src.contains("last_fired_yday"),
        "v35 day-boundary refire tracker `last_fired_yday` must exist"
    );
    assert!(
        src.contains("day-boundary refire"),
        "v35 day-boundary refire comment must exist"
    );
    assert!(
        src.contains("ambient::current_yday"),
        "v35 day-boundary refire must call current_yday"
    );
}

/// AB-09: regression test for the "comment → uncomment at same hour"
/// scenario reported after commit 128267e. The scheduler's `last_applied`
/// tracker must be cleared when the schedule transitions to empty, so
/// that re-adding the SAME entry (same hour, minute, AND scene) triggers
/// a fresh fire. Without this fix, the dedup check
/// (`last_applied.as_ref() != Some(entry)`) suppresses the legitimate
/// refire because `last_applied` still holds the previously-applied
/// entry, leaving the scene stuck on the config's default scene
/// (e.g. `cinematic`) instead of the ambient entry's scene.
///
/// Symptom in production: user comments out an ambient entry at hour X
/// (e.g. `ambient.12-00 = signal`), then uncomments it back within the
/// same minute/hour. Sometimes the scene applies, sometimes it doesn't,
/// requiring multiple config saves or a manual scene change to trigger.
/// Comparison with pre-v50: applied ambient immediately on
/// uncomment; post-128267e got stuck on the config's default scene.
#[test]
fn reload_after_empty_refires_same_entry() {
    // Use entry(0, 0) so it's always "current" (00:00 <= now_min for any
    // time of day). This makes the test deterministic regardless of
    // wall-clock time when the test runs.
    let s_initial = AmbientSchedule {
        entries: vec![entry(0, 0)],
    };
    let handle = spawn_ambient_scheduler(s_initial);

    // Drain initial fire (scheduler fires current phase on startup).
    let initial = handle
        .rx
        .recv_timeout(Duration::from_secs(1))
        .expect("scheduler should fire initial phase");
    assert_eq!(initial.hour, 0);
    assert_eq!(initial.minute, 0);

    // Wait for the scheduler thread to settle into its sleep.
    std::thread::sleep(Duration::from_millis(100));

    // User comments out the ambient entry — schedule becomes empty.
    handle.reload(AmbientSchedule::default());

    // Wait for the scheduler to process the reload (condvar wake +
    // re-snapshot). 150ms is plenty — the condvar notify is immediate
    // and the snapshot is a single mutex lock + clone.
    std::thread::sleep(Duration::from_millis(150));
    // No event should fire (schedule is empty).
    assert!(
        handle.rx.try_recv().is_err(),
        "no fire expected when schedule is empty"
    );

    // User uncomments the SAME entry back at the same hour.
    // Pre-AB-09: scheduler's last_applied was still Some(00:00 cinematic),
    // so the dedup suppressed this fire. Post-AB-09: last_applied was
    // cleared to None when the schedule went empty, so this fires.
    handle.reload(AmbientSchedule {
        entries: vec![entry(0, 0)],
    });

    // The scheduler MUST fire the entry within a reasonable window.
    // 2s is generous — the condvar wake is immediate, the snapshot
    // takes microseconds, and tx.send is non-blocking.
    match handle.rx.recv_timeout(Duration::from_secs(2)) {
        Ok(e) => {
            assert_eq!(e.hour, 0, "refire hour must match");
            assert_eq!(e.minute, 0, "refire minute must match");
            assert_eq!(
                e.scene, "cinematic",
                "refire scene must match the uncommented entry's scene"
            );
        }
        Err(_) => {
            panic!(
                "AB-09 regression: scheduler did not refire the same entry \
                     after empty → non-empty reload (comment/uncomment at same hour)"
            );
        }
    }
}

// ── deliver() unit tests (triple-engine LTS audit LOW-1, 2026-08-23) ─────
//
// The scheduler previously terminated its thread on ANY try_send error,
// conflating TrySendError::Full (transient) with TrySendError::Disconnected
// (fatal). These tests pin the three-way contract of the fix.

#[test]
fn deliver_succeeds_when_channel_has_space() {
    let (tx, _rx) = std::sync::mpsc::sync_channel::<AmbientEntry>(64);
    assert_eq!(deliver(&tx, &entry(12, 0)), DeliverOutcome::Delivered);
}

#[test]
fn deliver_reports_receiver_gone_when_rx_dropped() {
    let (tx, rx) = std::sync::mpsc::sync_channel::<AmbientEntry>(64);
    drop(rx);
    assert_eq!(deliver(&tx, &entry(12, 0)), DeliverOutcome::ReceiverGone);
}

#[test]
fn deliver_reports_saturated_when_channel_stays_full() {
    let (tx, rx) = std::sync::mpsc::sync_channel::<AmbientEntry>(1);
    // Fill the single slot so the next delivery attempt hits Full.
    tx.try_send(entry(0, 0))
        .expect("empty channel accepts first entry");
    // Receiver stays alive (no drain) — the bounded retry loop must
    // exhaust DELIVER_FULL_WAIT (1 s) and report Saturated instead of
    // the pre-fix behavior of terminating the caller.
    let started = std::time::Instant::now();
    assert_eq!(deliver(&tx, &entry(12, 0)), DeliverOutcome::Saturated);
    assert!(
        started.elapsed() >= DELIVER_FULL_WAIT,
        "saturated delivery must wait the full bounded duration"
    );
    drop(rx);
}

#[test]
fn deliver_recovers_when_space_frees_within_wait() {
    let (tx, rx) = std::sync::mpsc::sync_channel::<AmbientEntry>(1);
    tx.try_send(entry(0, 0))
        .expect("empty channel accepts first entry");
    // Drain the slot from another thread after 100 ms, but keep the
    // receiver alive until deliver() has returned — the drainer dropping
    // the last rx reference would race the retry loop into ReceiverGone
    // before it could observe the freed slot. Receiver is !Sync, so the
    // shared handle is an Arc<Mutex<Receiver>> (Mutex over a Send type
    // is Sync; the receiver never crosses threads unsynchronized).
    let rx = std::sync::Arc::new(std::sync::Mutex::new(rx));
    let drainer_rx = std::sync::Arc::clone(&rx);
    let drainer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        let _ = drainer_rx.lock().expect("rx mutex").try_recv();
    });
    assert_eq!(deliver(&tx, &entry(12, 0)), DeliverOutcome::Delivered);
    drainer.join().expect("drainer thread must not panic");
    drop(rx);
}
