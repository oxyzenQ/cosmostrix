// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

use std::process::Command;

const GITHUB_API_URL: &str = "https://api.github.com/repos/oxyzenq/cosmostrix/releases/latest";
const RELEASES_URL: &str = "https://github.com/oxyzenq/cosmostrix/releases/latest";

#[derive(Debug, PartialEq, Eq)]
enum UpdateStatus {
    UpToDate,
    UpdateAvailable,
    CurrentIsNewer,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

impl SemVer {
    fn parse(version: &str) -> Option<Self> {
        let version = version.trim();
        let version = version.strip_prefix('v').unwrap_or(version);
        let version = version
            .split_once('-')
            .map_or(version, |(stable, _)| stable);
        let mut parts = version.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

fn normalize_version(version: &str) -> String {
    let version = version.trim();
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

fn compare_versions(current: &str, latest: &str) -> UpdateStatus {
    match (SemVer::parse(current), SemVer::parse(latest)) {
        (Some(current), Some(latest)) if current == latest => UpdateStatus::UpToDate,
        (Some(current), Some(latest)) if current > latest => UpdateStatus::CurrentIsNewer,
        _ => UpdateStatus::UpdateAvailable,
    }
}

fn extract_tag_name(json: &str) -> Option<String> {
    let key = "\"tag_name\"";
    let rest = json.get(json.find(key)? + key.len()..)?;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn curl_failure(code: i32) -> &'static str {
    match code {
        6 => "DNS resolution failed",
        7 => "connection refused",
        28 => "network request timed out",
        35 => "SSL/TLS handshake failed",
        _ => "network request failed",
    }
}

fn http_failure(code: u16) -> &'static str {
    match code {
        403 => "GitHub API request was rate-limited or forbidden",
        404 => "no latest GitHub release found for oxyzenq/cosmostrix",
        _ => "GitHub API returned an unexpected error",
    }
}

pub fn check_update(current_version: &str) -> Result<(), String> {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--max-time",
            "15",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "User-Agent: cosmostrix",
            "--write-out",
            "\n%{http_code}",
            GITHUB_API_URL,
        ])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "curl is not available on PATH".to_string()
            } else {
                format!("failed to run curl: {e}")
            }
        })?;

    if !output.status.success() {
        return Err(curl_failure(output.status.code().unwrap_or(-1)).to_string());
    }

    let raw =
        String::from_utf8(output.stdout).map_err(|_| "response was not valid UTF-8".to_string())?;
    let (body, status) = raw
        .rsplit_once('\n')
        .ok_or_else(|| "GitHub API response was malformed".to_string())?;
    let status = status.trim().parse::<u16>().unwrap_or(0);
    if status != 200 {
        return Err(http_failure(status).to_string());
    }

    let latest_tag = extract_tag_name(body)
        .ok_or_else(|| "could not parse latest release tag from GitHub response".to_string())?;
    let status = match compare_versions(current_version, &latest_tag) {
        UpdateStatus::UpToDate => "up to date",
        UpdateStatus::UpdateAvailable => "update available",
        UpdateStatus::CurrentIsNewer => "current is newer than latest release",
    };

    println!("cosmostrix update check");
    println!("Current: {}", normalize_version(current_version));
    println!("Latest:  {}", normalize_version(&latest_tag));
    println!("Status:  {status}");
    println!("Source:  {RELEASES_URL}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tag_name() {
        assert_eq!(
            extract_tag_name(r#"{"tag_name":"v3.0.0"}"#),
            Some("v3.0.0".to_string())
        );
    }

    #[test]
    fn compares_versions() {
        assert_eq!(compare_versions("3.0.0", "v3.0.0"), UpdateStatus::UpToDate);
        assert_eq!(
            compare_versions("2.9.0", "v3.0.0"),
            UpdateStatus::UpdateAvailable
        );
        assert_eq!(
            compare_versions("3.1.0", "v3.0.0"),
            UpdateStatus::CurrentIsNewer
        );
    }
}
