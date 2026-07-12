// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon Supercharger — SIMD-accelerated dirty buffer clearing + buffer sizing.
//!
//! Pure Rust, zero C. Uses std::arch intrinsics for AVX2 (x86-64) with
//! automatic fallback to scalar fill on unsupported CPUs.
//!
//! The dirty_map BitVec in Frame is cleared every frame via clear_dirty().
//! This module provides a SIMD-accelerated path for the underlying byte
//! slice, writing 32 zeros per cycle instead of 1.

/// Output buffer size for interactive mode BufWriter (256 KB).
/// Larger buffer = fewer write() syscalls during frame flush.
pub const OUTPUT_BUFFER_SIZE: usize = 256 * 1024;

/// Clear a byte slice to zero using SIMD when available.
/// Used by Frame::clear_dirty() to reset the dirty_map BitVec's
/// underlying storage.
///
/// # Safety
/// This function is safe to call — the unsafe blocks are internal
/// and guarded by runtime feature detection.
#[allow(dead_code)]
#[inline]
pub fn clear_bytes_simd(bytes: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 is runtime-verified by is_x86_feature_detected!.
            // The unsafe block only uses unaligned stores which are safe on x86-64.
            unsafe {
                clear_bytes_avx2(bytes);
            }
            return;
        }
    }

    // Fallback: scalar fill
    bytes.fill(0);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn clear_bytes_avx2(bytes: &mut [u8]) {
    use std::arch::x86_64::_mm256_setzero_si256;
    use std::arch::x86_64::_mm256_storeu_si256;

    let zeros = _mm256_setzero_si256();
    let chunk_size = 32; // 256 bits = 32 bytes
    let chunks = bytes.len() / chunk_size;
    let remainder = bytes.len() % chunk_size;

    // Process 32 bytes at a time using AVX2 unaligned stores.
    // SAFETY: storeu_si256 handles unaligned addresses correctly on x86-64.
    // We iterate over chunks of 32 bytes, casting &mut [u8] to &mut [u8; 32]
    // via pointer arithmetic.
    for i in 0..chunks {
        let ptr = bytes.as_mut_ptr().add(i * chunk_size) as *mut std::arch::x86_64::__m256i;
        _mm256_storeu_si256(ptr, zeros);
    }

    // Handle remainder bytes with scalar fill.
    if remainder > 0 {
        let start = chunks * chunk_size;
        bytes[start..].fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_bytes_simd_fills_with_zeros() {
        let mut buf = vec![0xFFu8; 1024];
        clear_bytes_simd(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn clear_bytes_simd_handles_small_buffers() {
        let mut buf = vec![0xFFu8; 7];
        clear_bytes_simd(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn clear_bytes_simd_handles_exact_chunks() {
        let mut buf = vec![0xAAu8; 64]; // exactly 2 AVX2 chunks
        clear_bytes_simd(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn clear_bytes_simd_handles_empty() {
        let mut buf: Vec<u8> = vec![];
        clear_bytes_simd(&mut buf);
        assert!(buf.is_empty());
    }

    #[test]
    fn output_buffer_size_is_256kb() {
        assert_eq!(OUTPUT_BUFFER_SIZE, 256 * 1024);
    }
}
