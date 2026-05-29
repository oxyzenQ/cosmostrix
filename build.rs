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
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_BUILD");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_PROFILE");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_LTO");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_PANIC");
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_STRIP");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    let profile_name = detect_profile_name();

    let build_id = std::env::var("COSMOSTRIX_BUILD")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| infer_build_id(&profile_name));
    println!("cargo:rustc-env=COSMOSTRIX_BUILD={build_id}");

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
    std::env::var("COSMOSTRIX_PROFILE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var("CARGO_PROFILE_NAME")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(infer_profile_from_out_dir)
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

fn infer_build_id(profile_name: &str) -> String {
    let os_raw = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
    let os = match os_raw.as_str() {
        "macos" => "darwin",
        other => other,
    };

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_string());
    let features = std::env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();

    if arch == "x86_64" {
        if os == "linux" {
            let variant = if profile_name == "pro-linux-v4" {
                "v4"
            } else if profile_name == "pro-linux-v3" {
                "v3"
            } else if profile_name == "pro-linux-v2" {
                "v2"
            } else if profile_name == "pro-linux-v1" {
                "v1"
            } else if features.contains("avx512f") {
                "v4"
            } else if features.contains("avx2") {
                "v3"
            } else if features.contains("sse4.2") || features.contains("sse4_2") {
                "v2"
            } else {
                "v1"
            };
            format!("{os}-{arch}-{variant}")
        } else {
            format!("{os}-{arch}")
        }
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
