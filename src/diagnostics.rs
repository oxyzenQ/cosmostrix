// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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

/// Detect the CPU model string at runtime (e.g. "Intel(R) Core(TM) i7-12700K
/// CPU @ 3.60GHz" or "Apple M2 Pro").
///
/// Returns `None` on platforms without detection. Used by the benchmark
/// report's SYSTEM section so users can compare results across machines
/// without manually recording their hardware.
///
/// # Platform support
/// - **Linux**: parses `/proc/cpuinfo` for the `model name` field.
/// - **macOS**: queries `machdep.cpu.brand_string` via `sysctlbyname`.
/// - **Other**: returns `None`.
#[must_use]
pub fn cpu_model_string() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        linux_cpu_model()
    }
    #[cfg(target_os = "macos")]
    {
        macos_cpu_model()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn linux_cpu_model() -> Option<String> {
    let mut file = std::fs::File::open("/proc/cpuinfo").ok()?;
    let mut buf = String::new();
    std::io::Read::read_to_string(&mut file, &mut buf).ok()?;
    for line in buf.lines() {
        if let Some(rest) = line.strip_prefix("model name") {
            // Format: "model name      : Intel(R) Core(TM) ..."
            if let Some((_, value)) = rest.split_once(':') {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_cpu_model() -> Option<String> {
    #![allow(deprecated)]
    use std::ffi::CStr;
    use std::os::raw::c_char;

    // SAFETY: sysctlbyname with a known string key writes into our buffer.
    // machdep.cpu.brand_string returns a human-readable CPU model string.
    unsafe {
        let key = c"machdep.cpu.brand_string";
        let mut len: usize = 0;
        // First call with null/0 to get the required length.
        let rc0 = libc::sysctlbyname(
            key.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null(),
            0,
        );
        if rc0 != 0 || len == 0 {
            return None;
        }
        let mut buf: Vec<u8> = vec![0u8; len];
        let rc1 = libc::sysctlbyname(
            key.as_ptr(),
            buf.as_mut_ptr() as *mut _,
            &mut len,
            std::ptr::null(),
            0,
        );
        if rc1 != 0 {
            return None;
        }
        // Trim trailing NULs.
        while buf.last() == Some(&0) {
            buf.pop();
        }
        CStr::from_bytes_with_nul(&{
            let mut v = buf.clone();
            v.push(0);
            v
        })
        .ok()
        .and_then(|c| c.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    }
}
