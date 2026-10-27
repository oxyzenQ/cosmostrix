// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
//
//! The kernel-floor lane's entropy seed (night-dragon-maintenance-2).
//!
//! The 2.6.27 floor kernel predates the getrandom(2) syscall (3.17+),
//! so the rand crate's entropy path falls back to /dev/urandom - and
//! that fallback first waits for /dev/random to report pool credit,
//! polling it with an infinite timeout. A quiescent micro-VM (no disk,
//! no input devices, no network) credits no entropy on its own, so the
//! payload's first RNG seed wedges the bench leg until the outer boot
//! budget expires. A real 2.6.27 machine credits the pool from its own
//! interrupt entropy within seconds of boot; this toy initramfs has no
//! devices to jitter, so PID 1 runs this helper before the battery.
//!
//! The helper credits the pool through the kernel's own contract, the
//! RNDADDENTROPY ioctl: it mixes 512 bytes of guest-available noise
//! (urandom bytes folded with vCPU timing jitter, real runtime
//! scheduling noise of the host) into the input pool and credits 256
//! bits, then reports /proc/sys/kernel/random/entropy_avail as the
//! evidence row. Build shape: one file, std only, no cargo, compiled
//! by rustc directly in .github/workflows/supermassive-kernel.yml and
//! packed at .sandbox/entropy-seed; the init runs it when present and
//! gates the lane on the result. musl-static by construction.
//!
//! Deliberately absent: threads, sleeps and HashMaps. musl's
//! nanosleep routes through the clock_nanosleep syscall (2.6.28+) and
//! would fail with ENOSYS here; RandomState would seed through the
//! very fallback this helper exists to unblock.

use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_ulong, c_void};
use std::time::Instant;

extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, arg: c_ulong) -> c_int;
}

// linux/random.h uapi: RNDADDENTROPY = _IOW('R', 0x03, struct
// rand_pool_info), where struct rand_pool_info is
//   int entropy_count; int buf_size; __u32 buf[];
// so the ioctl number encodes direction 1 (write), size 8, type 'R',
// number 3 -> 0x40085203. entropy_count is in BITS, buf_size in bytes.
const RNDADDENTROPY: c_ulong = 0x4008_5203;
const SEED_BYTES: usize = 512;
const SEED_BITS: c_int = 256;
const ENTROPY_AVAIL: &str = "/proc/sys/kernel/random/entropy_avail";

#[repr(C)]
struct RandPoolInfo {
    entropy_count: c_int,
    buf_size: c_int,
    buf: [u32; SEED_BYTES / 4],
}

fn read_entropy_avail() -> Option<u64> {
    let mut s = String::new();
    File::open(ENTROPY_AVAIL)
        .and_then(|mut f| f.read_to_string(&mut s))
        .ok()?;
    s.trim().parse::<u64>().ok()
}

// Fold vCPU timing jitter (host scheduling noise under KVM - real
// runtime entropy of this guest) into the noise words.
fn fold_jitter(buf: &mut [u32; SEED_BYTES / 4]) {
    let mut acc: u32 = 0x9e37_79b9;
    for i in 0..buf.len() {
        let t0 = Instant::now();
        // A tiny busy slice so each sample measures real dispatch
        // latency, not a constant loop period.
        let mut x: u32 = acc;
        for _ in 0..64 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        }
        let t1 = Instant::now();
        let delta = t1.saturating_duration_since(t0);
        acc = acc
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            ^ x
            ^ delta.subsec_nanos()
            ^ ((delta.as_secs() as u32).wrapping_mul(2_654_435_761));
        buf[i] ^= acc.rotate_left((i as u32) % 31);
    }
}

fn main() {
    let before = read_entropy_avail();

    // The noise words: guest urandom bytes (real content, unknown
    // pre-credit quality) folded with timing jitter.
    let mut bytes = [0u8; SEED_BYTES];
    if let Err(e) = File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes)) {
        eprintln!("entropy-seed: cannot read /dev/urandom: {e}");
        std::process::exit(1);
    }
    let mut buf: [u32; SEED_BYTES / 4] = [0; SEED_BYTES / 4];
    for (w, chunk) in bytes.chunks_exact(4).enumerate() {
        buf[w] = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    fold_jitter(&mut buf);

    let mut pool = Box::new(RandPoolInfo {
        entropy_count: SEED_BITS,
        buf_size: SEED_BYTES as c_int,
        buf,
    });

    let dev = match File::open("/dev/random") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("entropy-seed: cannot open /dev/random: {e}");
            std::process::exit(1);
        }
    };
    let rc = unsafe {
        ioctl(
            dev.as_raw_fd(),
            RNDADDENTROPY,
            (&mut *pool as *mut RandPoolInfo) as *mut c_void as c_ulong,
        )
    };
    if rc < 0 {
        eprintln!("entropy-seed: RNDADDENTROPY ioctl failed (rc {rc})");
        std::process::exit(1);
    }

    let after = read_entropy_avail();
    println!(
        "entropy-seed: credited {} bits, entropy_avail {} -> {}",
        SEED_BITS,
        before.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
        after.map(|v| v.to_string()).unwrap_or_else(|| "?".into()),
    );
}
