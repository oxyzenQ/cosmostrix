// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: build script is a single-file cargo contract; its test suite must stay in-file for the standalone runner (rustc --edition 2021 --test build.rs) and grew past the cap through the hunt-2/hunt-3 hardening.

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
    let pgo_label = pgo_label(&build_id);

    println!("cargo:rustc-env=COSMOSTRIX_BUILD={build_id}");
    println!("cargo:rustc-env=COSMOSTRIX_OPTIMIZATION={optimization}");
    println!("cargo:rustc-env=COSMOSTRIX_CPU_BASELINE={cpu_baseline}");
    println!("cargo:rustc-env=COSMOSTRIX_TARGET_FEATURES={target_features_display}");
    println!("cargo:rustc-env=COSMOSTRIX_PGO={pgo_label}");

    // Commit-sha resolution chain (first hit wins):
    //   1. `git rev-parse --short=7 HEAD` — local/git checkouts.
    //   2. `GITHUB_SHA` env — CI environments.
    //   3. `.cargo_vcs_info.json` — crates.io tarball builds (`cargo
    //      install cosmostrix`): cargo embeds this file in the published
    //      tarball with the sha1 of the commit the crate was packaged
    //      from, so builds without a .git directory still recover the
    //      exact source revision (NIGHT-hunt-2 fix; previously the chain
    //      dead-ended here and `-V`/HUD showed an empty cid).
    let sha = git_short_sha()
        .or_else(|| env_short_sha("GITHUB_SHA"))
        .or_else(packaged_vcs_sha)
        .unwrap_or_default();
    println!("cargo:rustc-env=COSMOSTRIX_GIT_SHA={sha}");
    println!("cargo:rerun-if-changed=.cargo_vcs_info.json");

    let rustc_version = detect_rustc_version();
    println!("cargo:rustc-env=COSMOSTRIX_RUSTC_VERSION={rustc_version}");

    let metadata = detect_build_metadata(&profile_name);

    println!("cargo:rustc-env=COSMOSTRIX_LTO={}", metadata.lto);
    println!("cargo:rustc-env=COSMOSTRIX_PANIC={}", metadata.panic);
    println!("cargo:rustc-env=COSMOSTRIX_STRIP={}", metadata.strip);

    // Build timestamp: M/D/YYYY HH:MM (UTC at compile time).
    //
    // Why UTC and not local time? The previous implementation used
    // `chrono::Local::now()` which required `chrono` as a build-dependency
    // (a separate ~1.3s compile instance). For a build timestamp that is
    // purely informational (shown in `--version` output), UTC is fine and
    // lets us drop the chrono build-dep entirely. Runtime chrono was
    // also dropped (see the chrono note in Cargo.toml): the two production
    // call sites that needed wall-clock time now use libc::localtime_r /
    // libc::gmtime_r directly via the `src/clock` module and the
    // `local_secs_since_midnight` helper in `phase_predictor.rs`.
    //
    // Format matches the previous chrono `"%-m/%-d/%Y %H:%M"` output:
    // month and day are NOT zero-padded; year is 4 digits; hour and
    // minute ARE zero-padded to 2 digits. A "(UTC)" suffix is appended
    // to make the timezone explicit (the previous local-time output had
    // no designator, which was ambiguous).
    let build_time = format_build_time_utc();
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
    // NIGHT-blade-1: build ids and profile names carry the baseline as
    // an inner token (linux-amd64-v3-gnu, pro-linux-amd64-v3-musl), so
    // the claim check matches "-vN-" anywhere in the string OR a
    // trailing "-vN" (the legacy pro-linux-v3 shape and CI's
    // linux-x86_64-vN labels keep claiming too).
    if value.contains("-v4-") || value.ends_with("-v4") {
        Some("x86-64-v4")
    } else if value.contains("-v3-") || value.ends_with("-v3") {
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
    } else {
        // Below v3: no dedicated build profile (v1/v2 removed; only v3/v4
        // exist in .cargo/config.toml + Cargo.toml). Report as generic
        // baseline — users should use `pro` or `pro-native` for these CPUs.
        "x86-64-baseline"
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
        || profile_name.starts_with("pro-linux-amd64-v"))
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
        // v1/v2 build profiles were removed; CPUs below v3 get a generic
        // baseline label. Users with v1/v2-only CPUs should use `pro` or
        // `pro-native` (generic + native tuning) instead of a v-specific profile.
        "x86-64-baseline" if has_all_features(features, &["sse", "sse2"]) => {
            "x86-64 baseline (SSE/SSE2 — use `pro` or `pro-native`)"
        }
        "x86-64-baseline" => "x86-64 baseline (sub-SSE2 — use `pro` or `pro-native`)",
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

/// Returns `"yes"` if the current build was compiled with PGO profile data
/// (the `nitro-pgo` stage of `./scripts/build/build.sh pgo`), `"no"` otherwise.
///
/// The instrumentation stage (`nitro-pgo-instrument`) is NOT a PGO-optimized
/// build — it carries profiling overhead and is slower than a plain release
/// build. Only the final `nitro-pgo` stage, which consumes the collected
/// `.profdata`, should report `pgo: yes`.
fn pgo_label(build_id: &str) -> &'static str {
    if build_id == "nitro-pgo" {
        "yes"
    } else {
        "no"
    }
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
    eprintln!("cosmostrix CPU baseline mismatch:");
    eprintln!("  build: {build_id}");
    eprintln!("  profile: {profile_name}");
    eprintln!("  claimed baseline: {baseline}");
    eprintln!("  target_features: {}", format_target_features(features));
    eprintln!("  reason: {reason}");
    eprintln!();
    eprintln!("Use the cargo aliases (for example `cargo pro-linux-amd64-v3-gnu`) or set matching RUSTFLAGS explicitly.");
    std::process::exit(1);
}

fn env_short_sha(name: &str) -> Option<String> {
    normalize_short_sha(&std::env::var(name).ok()?)
}

/// Normalizes a full (40-hex) or already-short commit hash to the
/// 7-char lowercase short form used by `COSMOSTRIX_GIT_SHA`. Returns
/// `None` for empty or non-hex input so callers can fall through to
/// the next resolution step.
fn normalize_short_sha(v: &str) -> Option<String> {
    let v = v.trim();
    if v.is_empty() {
        return None;
    }
    let n = v.len().min(7);
    // NIGHT-ultimate-1: `&v[..n]` panicked the build script when the
    // env var carried multi-byte UTF-8 (the byte index landed mid-char).
    // `get(..n)` returns None on a non-char-boundary instead, falling
    // through to the next resolution step exactly like a non-hex value.
    let short = v.get(..n)?;
    if short.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(short.to_ascii_lowercase())
    } else {
        None
    }
}

/// Parses the `sha1` field out of a `.cargo_vcs_info.json` document.
/// Pure string extraction — build.rs must stay dependency-free (std
/// only), so there is no JSON crate here. The document shape is stable:
/// `{"git":{"sha1":"<40 hex>","dirty":bool},"path_in_vcs":""}`
/// (the `dirty` flag may be absent on clean publishes).
fn parse_vcs_sha_json(text: &str) -> Option<String> {
    let key = "\"sha1\"";
    let key_at = text.find(key)?;
    let after_key = &text[key_at + key.len()..];
    let colon_at = after_key.find(':')?;
    let value = after_key[colon_at + 1..].trim_start();
    let value = value.strip_prefix('"')?;
    let end = value.find('"')?;
    normalize_short_sha(&value[..end])
}

/// Third step of the commit-sha chain: read `.cargo_vcs_info.json` from
/// the package root. The build script's working directory is the package
/// root, and the file sits there in extracted registry sources (verified
/// against the real published cosmostrix v100.0.0 tarball). Returns
/// `None` when the file is missing (dev builds) or carries no usable sha.
fn packaged_vcs_sha() -> Option<String> {
    let text = std::fs::read_to_string(".cargo_vcs_info.json").ok()?;
    parse_vcs_sha_json(&text)
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

/// Build timestamp in `M/D/YYYY HH:MM (UTC)` format, computed from
/// `std::time::SystemTime` without chrono.
///
/// Replaces the previous `chrono::Local::now().format("%-m/%-d/%Y %H:%M")`
/// call so that `chrono` no longer needs to be a `[build-dependencies]`
/// entry (saves ~1.3s on clean release builds by avoiding a second
/// compile instance of chrono).
///
/// Returns "unknown" only if `SystemTime::now()` is somehow before
/// `UNIX_EPOCH` (which would indicate a broken system clock).
fn format_build_time_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let Ok(secs) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "unknown".to_string();
    };
    let total_secs: i64 = i64::try_from(secs.as_secs()).unwrap_or(0);
    format_unix_secs_as_build_time(total_secs)
}

/// Pure formatting function — takes unix-epoch seconds and returns
/// `M/D/YYYY HH:MM (UTC)`. Separated from `format_build_time_utc` so
/// the algorithm is unit-testable without depending on the wall clock.
///
/// Algorithm: split `total_secs` into days + seconds-of-day, then use
/// Howard Hinnant's `civil_from_days` algorithm
/// (http://howardhinnant.github.io/date_algorithms.html) to convert
/// days-since-epoch to (year, month, day). All arithmetic is on `i64`
/// to avoid unsigned-underflow issues when subtracting the 719468-day
/// shift constant.
fn format_unix_secs_as_build_time(total_secs: i64) -> String {
    let days_since_epoch = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;

    // Howard Hinnant's civil_from_days: converts days-since-1970-01-01
    // to (year, month, day) in the proleptic Gregorian calendar.
    // http://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };

    format!("{m}/{d}/{year} {hour:02}:{minute:02} (UTC)")
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: `cargo test` NEVER executes build-script tests — cargo treats
    // build.rs as a build dependency, not a test target. These tests only
    // run via the standalone runner (also documented in docs/MAINTENANCE.md):
    //
    //   rustc --edition 2021 --test build.rs -o /tmp/cosmostrix-build-script-tests
    //   /tmp/cosmostrix-build-script-tests
    //
    // The epoch constants below are date-verified via `date -u` so the
    // suite stays trustworthy as a civil-calendar regression guard.

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

            [profile.pro-linux-amd64-v3-gnu]
            inherits = "pro"
        "#;

        let profiles = parse_profiles(text);
        let mut effective = profile_defaults("pro-linux-amd64-v3-gnu");
        resolve_profile(
            "pro-linux-amd64-v3-gnu",
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

    #[test]
    fn pgo_label_recognizes_nitro_pgo_final_stage() {
        // Only the final PGO stage (profile-use) reports pgo=yes.
        // The instrumentation stage carries profiling overhead and is
        // slower than a plain release build, so it must NOT report pgo=yes.
        assert_eq!(pgo_label("nitro-pgo"), "yes");
        assert_eq!(pgo_label("nitro-pgo-instrument"), "no");
        assert_eq!(pgo_label("linux-amd64-v3"), "no");
        assert_eq!(pgo_label("linux-amd64-v3-musl"), "no");
        assert_eq!(pgo_label("pro-linux-amd64-v3-gnu"), "no");
        assert_eq!(pgo_label("unknown"), "no");
        assert_eq!(pgo_label(""), "no");
    }

    #[test]
    fn claimed_x86_baseline_matches_blade1_name_shapes() {
        // NIGHT-blade-1: ids/profiles carry the baseline as an inner
        // token; both the new (linux-amd64-v3-gnu) and the legacy
        // (pro-linux-v3) shapes must claim their baseline, and
        // baseline-less ids must not claim.
        assert_eq!(
            claimed_x86_baseline("linux-amd64-v3-gnu"),
            Some("x86-64-v3")
        );
        assert_eq!(
            claimed_x86_baseline("linux-amd64-v4-gnu"),
            Some("x86-64-v4")
        );
        assert_eq!(
            claimed_x86_baseline("linux-amd64-v3-musl"),
            Some("x86-64-v3")
        );
        assert_eq!(
            claimed_x86_baseline("linux-amd64-v4-musl"),
            Some("x86-64-v4")
        );
        assert_eq!(
            claimed_x86_baseline("pro-linux-amd64-v3-gnu"),
            Some("x86-64-v3")
        );
        assert_eq!(claimed_x86_baseline("pro-linux-v3"), Some("x86-64-v3"));
        assert_eq!(claimed_x86_baseline("linux-amd64-musl"), None);
        assert_eq!(claimed_x86_baseline("pro"), None);
    }

    #[test]
    fn build_time_format_matches_known_unix_epochs() {
        // UNIX epoch: 1970-01-01 00:00:00 UTC.
        assert_eq!(format_unix_secs_as_build_time(0), "1/1/1970 00:00 (UTC)");

        // 2000-01-01 00:00:00 UTC = 946_684_800 seconds since epoch.
        // Computed via: date -u -d '2000-01-01 00:00:00' +%s
        assert_eq!(
            format_unix_secs_as_build_time(946_684_800),
            "1/1/2000 00:00 (UTC)"
        );

        // 2024-02-29 12:34:00 UTC = 1_709_210_040 seconds since epoch.
        // Leap-day boundary check — Feb 29 must not roll to Mar 1.
        // Computed via: date -u -d '2024-02-29 12:34:00' +%s
        // (NIGHT-hunt-3: the previous constant 1_709_210_440 actually
        // decodes to 2024-02-29 12:40:40 UTC, verified via `date -u
        // -d @1709210440`. The date-verified value is 1_709_210_040.)
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_040),
            "2/29/2024 12:34 (UTC)"
        );

        // 2026-08-04 15:30:00 UTC = 1_785_857_400 seconds since epoch.
        // Computed via: date -u -d '2026-08-04 15:30:00' +%s
        // (NIGHT-hunt-3: the previous constant 1_787_930_200 actually
        // decodes to 2026-08-28 15:16:40 UTC, verified via `date -u
        // -d @1787930200`. The date-verified value is 1_785_857_400.)
        assert_eq!(
            format_unix_secs_as_build_time(1_785_857_400),
            "8/4/2026 15:30 (UTC)"
        );
    }

    #[test]
    fn build_time_format_truncates_sub_minute_seconds() {
        // The formatter renders only H:MM (no seconds field), so
        // sub-minute seconds must be TRUNCATED toward the past minute,
        // never rounded up into the next minute. 12:34:00 + 59 seconds
        // is still rendered as 12:34.
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_040 + 59),
            "2/29/2024 12:34 (UTC)"
        );

        // One second past a whole minute rolls the rendered minute
        // forward only at the :00 boundary (12:34:60 == 12:35:00).
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_040 + 60),
            "2/29/2024 12:35 (UTC)"
        );
    }

    #[test]
    fn vcs_info_parser_extracts_sha_from_published_documents() {
        // Exact shape of the real cosmostrix v100.0.0 tarball document
        // (downloaded from crates.io and inspected while fixing
        // NIGHT-hunt-2). A clean publish carries only the sha1:
        let clean = concat!(
            "{\"git\":{\"sha1\":\"6c51147732b79313b29f084b4da33dbd55b0ba82\"},",
            "\"path_in_vcs\":\"\"}"
        );
        assert_eq!(parse_vcs_sha_json(clean), Some("6c51147".to_string()));

        // A dirty publish (`cargo package --allow-dirty`) keeps the sha1
        // of HEAD and additionally sets the dirty flag — the sha must
        // still be recoverable.
        let dirty = concat!(
            "{\"git\":{\"sha1\":\"fa701c653ebee39e88e9b14453818630016f9a0f\",",
            "\"dirty\":true},\"path_in_vcs\":\"\"}"
        );
        assert_eq!(parse_vcs_sha_json(dirty), Some("fa701c6".to_string()));

        // Uppercase hex input is normalized to lowercase.
        let upper = "{\"git\":{\"sha1\":\"ABCDEF0123456\"},\"path_in_vcs\":\"\"}";
        assert_eq!(parse_vcs_sha_json(upper), Some("abcdef0".to_string()));
    }

    #[test]
    fn vcs_info_parser_rejects_malformed_documents() {
        // No sha1 key at all.
        assert_eq!(
            parse_vcs_sha_json("{\"git\":{},\"path_in_vcs\":\"\"}"),
            None
        );
        // Non-hex sha1 value.
        assert_eq!(
            parse_vcs_sha_json("{\"git\":{\"sha1\":\"not-a-hash\"},\"path_in_vcs\":\"\"}"),
            None
        );
        // Empty sha1 value.
        assert_eq!(
            parse_vcs_sha_json("{\"git\":{\"sha1\":\"\"},\"path_in_vcs\":\"\"}"),
            None
        );
        // Truncated document cut off mid-value.
        assert_eq!(parse_vcs_sha_json("{\"git\":{\"sha1\":\"6c51147"), None);
    }

    #[test]
    fn normalize_short_sha_truncates_validates_and_lowercases() {
        assert_eq!(
            normalize_short_sha("6c51147732b79313b29f084b4da33dbd55b0ba82"),
            Some("6c51147".to_string())
        );
        // Already-short input passes through unchanged.
        assert_eq!(normalize_short_sha("6c51147"), Some("6c51147".to_string()));
        // Surrounding whitespace is trimmed.
        assert_eq!(
            normalize_short_sha(" 6c51147 \n"),
            Some("6c51147".to_string())
        );
        // Empty / whitespace-only / non-hex inputs fall through to None.
        assert_eq!(normalize_short_sha(""), None);
        assert_eq!(normalize_short_sha("   "), None);
        assert_eq!(normalize_short_sha("zzzzzzz"), None);
    }

    #[test]
    fn build_time_format_handles_negative_seconds_gracefully() {
        // Pre-epoch timestamps (negative seconds) should still produce
        // a valid proleptic Gregorian date via the algorithm's signed
        // arithmetic, not panic or underflow.
        // 1969-12-31 23:59:00 UTC = -60 seconds.
        let result = format_unix_secs_as_build_time(-60);
        assert!(
            result.ends_with("(UTC)"),
            "negative-epoch result should still be (UTC)-suffixed: {result}"
        );
        assert!(
            result.contains("1969"),
            "negative-epoch result should land in 1969: {result}"
        );
    }
}
