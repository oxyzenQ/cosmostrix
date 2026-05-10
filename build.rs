fn main() {
    println!("cargo:rerun-if-env-changed=COSMOSTRIX_BUILD");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");

    let build_id = if let Ok(v) = std::env::var("COSMOSTRIX_BUILD") {
        if !v.is_empty() {
            v
        } else {
            infer_build_id()
        }
    } else {
        infer_build_id()
    };

    println!("cargo:rustc-env=COSMOSTRIX_BUILD={}", build_id);

    let sha = git_short_sha()
        .or_else(|| env_short_sha("GITHUB_SHA"))
        .unwrap_or_default();
    println!("cargo:rustc-env=COSMOSTRIX_GIT_SHA={}", sha);
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

fn infer_build_id() -> String {
    let os_raw = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| "unknown".to_string());
    let os = match os_raw.as_str() {
        "macos" => "darwin",
        other => other,
    };

    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_string());
    let features = std::env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();

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
            format!("{os}-{arch}-{variant}")
        } else {
            format!("{os}-{arch}")
        }
    } else {
        format!("{os}-{arch}-native")
    }
}
