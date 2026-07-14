// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::{HashMap, HashSet};
use std::path::Path;

const PROFILE_KEYS: [&str; 7] = [
    "lto",
    "panic",
    "strip",
    "opt-level",
    "codegen-units",
    "overflow-checks",
    "debug",
];

#[derive(Debug, Default)]
struct Profile {
    inherits: Option<String>,
    values: HashMap<String, String>,
}

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    emit_git_rerun_triggers();
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_BUILD");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_PROFILE");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_LTO");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_PANIC");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_STRIP");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_FEATURE");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    let profile_name = detect_profile_name();
    let target_features = target_features();
    let target_features_display = format_target_features(&target_features);

    let build_id = std::env::var("COSMOSTRIX_BUILD")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| infer_build_id(&target_features));
    let cpu_baseline = cpu_baseline(&build_id, &profile_name, &target_features);
    verify_cpu_baseline(&build_id, &profile_name, cpu_baseline, &target_features);
    let optimization = optimization_label(&build_id, cpu_baseline, &target_features);

    println!("cargo:rustc-env=COSMOSTRIX_BUILD={build_id}");
    println!("cargo:rustc-env=COSMOSTRIX_OPTIMIZATION={optimization}");
    println!("cargo:rustc-env=COSMOSTRIX_CPU_BASELINE={cpu_baseline}");
    println!("cargo:rustc-env=COSMOSTRIX_TARGET_FEATURES={target_features_display}");

    let sha = git_short_sha()
        .or_else(|| env_short_sha("GITHUB_SHA"))
        .unwrap_or_default();
    println!("cargo:rustc-env=COSMOSTRIX_GIT_SHA={sha}");

    let rustc_version = detect_rustc_version();
    println!("cargo:rustc-env=COSMOSTRIX_RUSTC_VERSION={rustc_version}");

    let metadata = detect_build_metadata(&profile_name);

    println!("cargo:rustc-env=COSMOSTRIX_LTO={}", metadata.lto);
    println!("cargo:rustc-env=COSMOSTRIX_PANIC={}", metadata.panic);
    println!("cargo:rustc-env=COSMOSTRIX_STRIP={}", metadata.strip);

    // Build timestamp: MM/DD/YYYY HH:MM (local time at compile).
    // Uses chrono (already a dependency) for cross-platform local time.
    // This is baked into the binary at compile time — not runtime.
    let now = chrono::Local::now();
    let build_time = now.format("%-m/%-d/%Y %H:%M").to_string();
    println!("cargo:rustc-env=COSMOSTRIX_BUILD_TIME={build_time}");
}

fn emit_git_rerun_triggers() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/packed-refs");

    let Ok(head) = std::fs::read_to_string(".git/HEAD") else {
        return;
    };
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref: ") {
        println!("cargo:rerun-if-changed=.git/{reference}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildMetadata {
    lto: String,
    panic: String,
    strip: String,
}

fn detect_build_metadata(profile_name: &str) -> BuildMetadata {
    let profile = read_effective_profile(profile_name);

    let lto = std::env::var("COSMOSTRIX_LTO")
        .ok()
        .or_else(|| profile.get("lto").cloned())
        .unwrap_or_else(|| "off".to_string());
    let panic = std::env::var("COSMOSTRIX_PANIC")
        .ok()
        .or_else(|| profile.get("panic").cloned())
        .unwrap_or_else(|| "unwind".to_string());
    let strip = std::env::var("COSMOSTRIX_STRIP")
        .ok()
        .or_else(|| profile.get("strip").cloned())
        .unwrap_or_else(|| "no".to_string());

    BuildMetadata {
        lto: normalize_lto(&lto),
        panic: normalize_panic(&panic),
        strip: normalize_strip(&strip),
    }
}

fn detect_profile_name() -> String {
    if let Some(profile) = std::env::var("COSMOSTRIX_PROFILE")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        return profile;
    }

    let cargo_profile = std::env::var("CARGO_PROFILE_NAME")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let out_dir_profile = infer_profile_from_out_dir();

    if let Some(profile) = cargo_profile {
        let cargo_profile_is_generic = profile == "release" || profile == "debug";
        if !cargo_profile_is_generic || out_dir_profile.as_deref() == Some(profile.as_str()) {
            return profile;
        }
    }

    out_dir_profile
        .or_else(|| {
            std::env::var("PROFILE")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "release".to_string())
}

fn infer_profile_from_out_dir() -> Option<String> {
    let out_dir = std::env::var_os("OUT_DIR")?;
    let components: Vec<_> = Path::new(&out_dir)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    components.windows(2).find_map(|pair| {
        if pair[1] == "build" && !pair[0].is_empty() {
            Some(pair[0].clone())
        } else {
            None
        }
    })
}

fn read_effective_profile(profile_name: &str) -> HashMap<String, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let cargo_toml = Path::new(&manifest_dir).join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(cargo_toml) else {
        return profile_defaults(profile_name);
    };

    let profiles = parse_profiles(&text);
    let mut effective = profile_defaults(profile_name);
    let mut resolving = HashSet::new();
    resolve_profile(profile_name, &profiles, &mut resolving, &mut effective);
    effective
}

fn profile_defaults(profile_name: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    if profile_name == "dev" {
        values.insert("opt-level".to_string(), "0".to_string());
        values.insert("debug".to_string(), "true".to_string());
        values.insert("codegen-units".to_string(), "256".to_string());
        values.insert("overflow-checks".to_string(), "true".to_string());
    } else {
        values.insert("opt-level".to_string(), "3".to_string());
        values.insert("debug".to_string(), "false".to_string());
        values.insert("codegen-units".to_string(), "16".to_string());
        values.insert("overflow-checks".to_string(), "false".to_string());
    }
    values.insert("lto".to_string(), "off".to_string());
    values.insert("panic".to_string(), "unwind".to_string());
    values.insert("strip".to_string(), "no".to_string());
    values
}

fn parse_profiles(text: &str) -> HashMap<String, Profile> {
    let mut profiles: HashMap<String, Profile> = HashMap::new();
    let mut current_profile: Option<String> = None;

    for line in text.lines() {
        let line = strip_toml_comment(line).trim().to_string();
        if line.is_empty() {
            continue;
        }

        if let Some(table) = line
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .map(str::trim)
        {
            current_profile = table.strip_prefix("profile.").map(ToOwned::to_owned);
            continue;
        }

        let Some(profile_name) = current_profile.as_deref() else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = parse_toml_scalar(value.trim());

        let profile = profiles.entry(profile_name.to_string()).or_default();
        if key == "inherits" {
            profile.inherits = Some(value);
        } else if PROFILE_KEYS.contains(&key) {
            profile.values.insert(key.to_string(), value);
        }
    }

    profiles
}

fn resolve_profile(
    profile_name: &str,
    profiles: &HashMap<String, Profile>,
    resolving: &mut HashSet<String>,
    effective: &mut HashMap<String, String>,
) {
    if !resolving.insert(profile_name.to_string()) {
        return;
    }

    if let Some(profile) = profiles.get(profile_name) {
        if let Some(parent) = &profile.inherits {
            resolve_profile(parent, profiles, resolving, effective);
        }
        for (key, value) in &profile.values {
            effective.insert(key.clone(), value.clone());
        }
    }

    resolving.remove(profile_name);
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_quote = false;
    let mut escaped = false;

    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_quote => escaped = true,
            '"' => in_quote = !in_quote,
            '#' if !in_quote => return &line[..idx],
            _ => {}
        }
    }

    line
}

fn parse_toml_scalar(value: &str) -> String {
    let value = value.trim();
    if let Some(unquoted) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        unquoted.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_lto(value: &str) -> String {
    match value.trim().trim_matches('"').to_ascii_lowercase().as_str() {
        "true" | "fat" => "fat",
        "thin" => "thin",
        "false" | "off" | "no" | "n" | "none" => "off",
        _ => "off",
    }
    .to_string()
}

fn normalize_panic(value: &str) -> String {
    match value.trim().trim_matches('"').to_ascii_lowercase().as_str() {
        "abort" => "abort",
        _ => "unwind",
    }
    .to_string()
}

fn normalize_strip(value: &str) -> String {
    match value.trim().trim_matches('"').to_ascii_lowercase().as_str() {
        "true" | "symbols" | "yes" => "yes",
        "debuginfo" => "debuginfo",
        "false" | "none" | "no" => "no",
        _ => "no",
    }
    .to_string()
}

fn target_features() -> HashSet<String> {
    std::env::var("CARGO_CFG_TARGET_FEATURE")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn format_target_features(features: &HashSet<String>) -> String {
    let mut features: Vec<_> = features.iter().map(String::as_str).collect();
    features.sort_unstable();
    if features.is_empty() {
        "none".to_string()
    } else {
        features.join(",")
    }
}

fn cpu_baseline(build_id: &str, profile_name: &str, features: &HashSet<String>) -> &'static str {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if arch == "aarch64" {
        return "aarch64-native";
    }
    if arch != "x86_64" {
        return "unknown";
    }

    claimed_x86_baseline(build_id)
        .or_else(|| claimed_x86_baseline(profile_name))
        .unwrap_or_else(|| detected_x86_baseline(features))
}

fn claimed_x86_baseline(value: &str) -> Option<&'static str> {
    if value.ends_with("-v4") {
        Some("x86-64-v4")
    } else if value.ends_with("-v3") {
        Some("x86-64-v3")
    } else {
        None
    }
}

fn detected_x86_baseline(features: &HashSet<String>) -> &'static str {
    if has_all_features(
        features,
        &["avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl"],
    ) {
        "x86-64-v4"
    } else if has_all_features(features, &["avx2", "bmi2", "fma"]) {
        "x86-64-v3"
    } else if has_all_features(features, &["sse4.2", "popcnt"]) {
        "x86-64-v2"
    } else {
        "x86-64-v1"
    }
}

fn verify_cpu_baseline(
    build_id: &str,
    profile_name: &str,
    baseline: &str,
    features: &HashSet<String>,
) {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let official_linux_x86 = (build_id.starts_with("linux-amd64-")
        || profile_name.starts_with("pro-linux-v")
        || profile_name.starts_with("pro-linux-musl"))
        && os == "linux"
        && arch == "x86_64";

    if !official_linux_x86 {
        return;
    }

    let missing = missing_required_features(baseline, features);
    if !missing.is_empty() {
        fail_cpu_baseline(
            build_id,
            profile_name,
            baseline,
            features,
            &format!(
                "missing compile-time target features: {}",
                missing.join(",")
            ),
        );
    }
}

fn optimization_label(build_id: &str, baseline: &str, features: &HashSet<String>) -> &'static str {
    if is_native_tuned_build(build_id) {
        return "native CPU tuned build";
    }

    // musl builds: same CPU baseline + static linking note
    let is_musl = build_id.ends_with("-musl");

    match baseline {
        "x86-64-v4"
            if has_all_features(
                features,
                &["avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl"],
            ) =>
        {
            if is_musl {
                "x86-64-v4 baseline (AVX-512) + musl static"
            } else {
                "x86-64-v4 baseline (AVX-512)"
            }
        }
        "x86-64-v3" if has_all_features(features, &["avx", "avx2", "bmi1", "bmi2", "fma"]) => {
            if is_musl {
                "x86-64-v3 baseline (AVX/AVX2/BMI1/BMI2/FMA) + musl static"
            } else {
                "x86-64-v3 baseline (AVX/AVX2/BMI1/BMI2/FMA)"
            }
        }
        "x86-64-v2"
            if has_all_features(features, &["sse3", "ssse3", "sse4.1", "sse4.2", "popcnt"]) =>
        {
            "x86-64-v2 baseline (SSE3/SSSE3/SSE4.1/SSE4.2/POPCNT)"
        }
        "x86-64-v1" if has_all_features(features, &["sse", "sse2"]) => "x86-64 baseline (SSE/SSE2)",
        "aarch64-native" => "aarch64 target build",
        "unknown" => "generic target build",
        _ => "generic CPU baseline build",
    }
}

fn is_native_tuned_build(build_id: &str) -> bool {
    if build_id.starts_with("android-") {
        return false;
    }

    let rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();
    let encoded_rustflags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    rustflags.contains("target-cpu=native") || encoded_rustflags.contains("target-cpu=native")
}

fn missing_required_features(baseline: &str, features: &HashSet<String>) -> Vec<&'static str> {
    let required: &[&str] = match baseline {
        "x86-64-v4" => &["avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl"],
        "x86-64-v3" => &["avx2", "bmi2", "fma"],
        _ => &[],
    };

    required
        .iter()
        .copied()
        .filter(|feature| !features.contains(*feature))
        .collect()
}

fn has_all_features(features: &HashSet<String>, required: &[&str]) -> bool {
    required.iter().all(|feature| features.contains(*feature))
}

fn fail_cpu_baseline(
    build_id: &str,
    profile_name: &str,
    baseline: &str,
    features: &HashSet<String>,
    reason: &str,
) -> ! {
    eprintln!("Cosmostrix CPU baseline mismatch:");
    eprintln!("  build: {build_id}");
    eprintln!("  profile: {profile_name}");
    eprintln!("  claimed baseline: {baseline}");
    eprintln!("  target_features: {}", format_target_features(features));
    eprintln!("  reason: {reason}");
    eprintln!();
    eprintln!("Use the cargo aliases (for example `cargo pro-linux-v3`) or set matching RUSTFLAGS explicitly.");
    std::process::exit(1);
}

fn env_short_sha(name: &str) -> Option<String> {
    let v = std::env::var(name).ok()?;
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    let n = v.len().min(7);
    let short = &v[..n];
    if short.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(short.to_ascii_lowercase())
    } else {
        None
    }
}

fn git_short_sha() -> Option<String> {
    use std::process::Command;

    let out = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(s.to_ascii_lowercase())
    } else {
        None
    }
}

fn infer_build_id(features: &HashSet<String>) -> String {
    let os_raw = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
    let os = match os_raw.as_str() {
        "macos" => "darwin",
        other => other,
    };

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_string());
    // Linux uses normalized archive naming: "amd64" for x86_64 and bare
    // "aarch64" (no -native suffix) for arm64, matching release asset names.
    // Other platforms keep their original arch label and -native suffix.
    let arch_label = if os == "linux" && arch == "x86_64" {
        "amd64"
    } else {
        arch.as_str()
    };
    // Detect libc variant (gnu = glibc/dynamic, musl = static) for Linux builds.
    // This matches the user-facing build label convention: linux-amd64-vN-gnu/musl
    let env_suffix = if os == "linux" {
        let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        if env == "musl" {
            "-musl"
        } else if env == "gnu" {
            "-gnu"
        } else {
            ""
        }
    } else {
        ""
    };

    if arch == "x86_64" {
        if os == "linux" {
            let variant = if features.contains("avx512f") {
                "v4"
            } else if features.contains("avx2") {
                "v3"
            } else if features.contains("sse4.2") || features.contains("sse4_2") {
                "v2"
            } else {
                "v1"
            };
            format!("{os}-{arch_label}-{variant}{env_suffix}")
        } else {
            format!("{os}-{arch}")
        }
    } else if os == "linux" {
        format!("{os}-{arch_label}{env_suffix}")
    } else {
        format!("{os}-{arch}-native")
    }
}

fn detect_rustc_version() -> String {
    use std::process::Command;

    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_inherited_profile_values() {
        let text = r#"
            [profile.release]
            lto = "fat"
            panic = "unwind"
            strip = true

            [profile.pro]
            inherits = "release"
            codegen-units = 1

            [profile.pro-linux-v3]
            inherits = "pro"
        "#;

        let profiles = parse_profiles(text);
        let mut effective = profile_defaults("pro-linux-v3");
        resolve_profile(
            "pro-linux-v3",
            &profiles,
            &mut HashSet::new(),
            &mut effective,
        );

        assert_eq!(effective.get("lto").map(String::as_str), Some("fat"));
        assert_eq!(effective.get("panic").map(String::as_str), Some("unwind"));
        assert_eq!(effective.get("strip").map(String::as_str), Some("true"));
        assert_eq!(
            effective.get("codegen-units").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn normalizes_metadata_values() {
        assert_eq!(normalize_lto("true"), "fat");
        assert_eq!(normalize_lto("\"thin\""), "thin");
        assert_eq!(normalize_lto("no"), "off");
        assert_eq!(normalize_panic("abort"), "abort");
        assert_eq!(normalize_panic("anything-else"), "unwind");
        assert_eq!(normalize_strip("symbols"), "yes");
        assert_eq!(normalize_strip("debuginfo"), "debuginfo");
        assert_eq!(normalize_strip("false"), "no");
    }
}
