// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Runtime CPU feature detection and binary variant reporting.
//!
//! Detects the CPU's instruction set capabilities at runtime and reports
//! the x86_64 microarchitecture level (v1–v4) or aarch64-native.

/// Detected CPU information.
pub struct CpuInfo {
    /// Microarchitecture variant, e.g. "x86_64-v3" or "aarch64-native".
    pub variant: &'static str,
    /// Dispatch description, always "static optimized build".
    pub dispatch: &'static str,
    /// Build-time variant from COSMOSTRIX_BUILD env.
    pub build_variant: &'static str,
}

/// Detect CPU info at runtime.
#[inline]
pub fn detect_cpu_info() -> CpuInfo {
    CpuInfo {
        variant: detect_variant(),
        dispatch: "static optimized build",
        build_variant: option_env!("COSMOSTRIX_BUILD").unwrap_or("unknown"),
    }
}

#[cfg(target_arch = "x86_64")]
fn detect_variant() -> &'static str {
    let has_sse42 = is_x86_feature_detected!("sse4.2");
    let has_avx2 = is_x86_feature_detected!("avx2");
    let has_bmi2 = is_x86_feature_detected!("bmi2");
    let has_fma = is_x86_feature_detected!("fma");
    let has_avx512f = is_x86_feature_detected!("avx512f");

    if has_avx512f {
        "x86_64-v4"
    } else if has_avx2 && has_bmi2 && has_fma {
        "x86_64-v3"
    } else if has_sse42 {
        "x86_64-v2"
    } else {
        "x86_64-v1"
    }
}

#[cfg(target_arch = "aarch64")]
fn detect_variant() -> &'static str {
    "aarch64-native"
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn detect_variant() -> &'static str {
    "unknown"
}
