// Copyright (c) 2026 rezky_nightky

//! Runtime CPU feature detection and binary variant reporting.
//!
//! Detects the CPU's instruction set capabilities at runtime and reports
//! the x86_64 microarchitecture level (v1–v4) or aarch64-native.

/// Detected CPU information.
pub struct CpuInfo {
    /// Microarchitecture variant, e.g. "x86_64-v3" or "aarch64-native".
    pub variant: &'static str,
    /// Detected feature names, e.g. ["AVX2", "BMI2", "FMA"].
    pub features: Vec<&'static str>,
    /// Dispatch description, always "static optimized build".
    pub dispatch: &'static str,
    /// Build-time variant from COSMOSTRIX_BUILD env.
    pub build_variant: &'static str,
}

/// Detect CPU info at runtime.
#[inline]
pub fn detect_cpu_info() -> CpuInfo {
    let (variant, features) = detect_variant_and_features();
    CpuInfo {
        variant,
        features,
        dispatch: "static optimized build",
        build_variant: option_env!("COSMOSTRIX_BUILD").unwrap_or("unknown"),
    }
}

/// Return a slash-separated feature string for display (e.g. "AVX2/BMI2/FMA").
#[inline]
pub fn feature_string(features: &[&str]) -> String {
    features.join("/")
}

#[cfg(target_arch = "x86_64")]
fn detect_variant_and_features() -> (&'static str, Vec<&'static str>) {
    let mut features: Vec<&'static str> = Vec::new();

    let has_sse42 = is_x86_feature_detected!("sse4.2");
    let has_avx2 = is_x86_feature_detected!("avx2");
    let has_bmi2 = is_x86_feature_detected!("bmi2");
    let has_fma = is_x86_feature_detected!("fma");
    let has_avx512f = is_x86_feature_detected!("avx512f");

    if has_sse42 {
        features.push("SSE4.2");
    }
    if has_avx2 {
        features.push("AVX2");
    }
    if has_bmi2 {
        features.push("BMI2");
    }
    if has_fma {
        features.push("FMA");
    }
    if has_avx512f {
        features.push("AVX-512F");
    }

    let variant = if has_avx512f {
        "x86_64-v4"
    } else if has_avx2 && has_bmi2 && has_fma {
        "x86_64-v3"
    } else if has_sse42 {
        "x86_64-v2"
    } else {
        "x86_64-v1"
    };

    (variant, features)
}

#[cfg(target_arch = "aarch64")]
fn detect_variant_and_features() -> (&'static str, Vec<&'static str>) {
    ("aarch64-native", vec!["NEON"])
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn detect_variant_and_features() -> (&'static str, Vec<&'static str>) {
    ("unknown", vec![])
}
