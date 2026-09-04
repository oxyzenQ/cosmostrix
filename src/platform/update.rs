// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use crate::output::println_safe;
use std::process::Command;

const GITHUB_API_URL: &str = "https://api.github.com/repos/oxyzenQ/cosmostrix/releases/latest";
const RELEASES_URL: &str = "https://github.com/oxyzenQ/cosmostrix/releases/latest";

/// NIGHT-hunter-7 (owner hunt 2026-09-05): error when no HTTP fetcher is
/// installed. Actionable on purpose — names both accepted tools AND the
/// manual URL, so a curl-less system (Alpine/busybox, minimal containers,
/// older Windows, hardened distros) never hits a dead end.
const NO_FETCHER_MSG: &str = "neither curl nor wget is available on PATH; install either via your package manager, or check the latest release manually at https://github.com/oxyzenQ/cosmostrix/releases/latest";

/// Canonical GitHub API URL for update checks (exposed for metadata tests).
#[cfg(test)]
pub(crate) const CANONICAL_GITHUB_API_URL: &str = GITHUB_API_URL;

/// Canonical releases URL (exposed for metadata tests).
#[cfg(test)]
pub(crate) const CANONICAL_RELEASES_URL: &str = RELEASES_URL;

/// Canonical no-fetcher error (exposed for metadata tests).
#[cfg(test)]
pub(crate) const CANONICAL_NO_FETCHER_MSG: &str = NO_FETCHER_MSG;

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

/// GNU wget exit classes: 8 = the server responded 4xx/5xx (wget with
/// `-O -` only exits 0 on success responses), 4 = network failure.
/// busybox wget (Alpine) collapses every failure to 1 — the generic
/// arm keeps the message accurate for both implementations.
fn wget_failure(code: i32) -> &'static str {
    match code {
        8 => "GitHub API returned an unexpected error",
        _ => "network request failed",
    }
}

fn http_failure(code: u16) -> &'static str {
    match code {
        403 => "GitHub API request was rate-limited or forbidden",
        404 => "no latest GitHub release found for oxyzenQ/cosmostrix",
        _ => "GitHub API returned an unexpected error",
    }
}

/// curl argv for the release check. The `--write-out "\n%{http_code}"
/// suffix appends the HTTP status as a final line so non-200 responses
/// are classified exactly (rate-limit 403, no-release 404).
fn curl_args() -> [&'static str; 10] {
    [
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
    ]
}

/// wget argv for the release check — the NIGHT-hunter-7 fallback for
/// systems without curl. Flag set is the intersection of GNU wget and
/// busybox wget so Alpine/containers work unchanged: `-q` silent,
/// `-O -` body to stdout, `-T 15` network timeout.
///
/// Trade-off (documented, accepted): GNU wget has no curl-style
/// total-time cap and retries connection errors up to 20 times by
/// default; `--tries`/`--waitretry` are NOT busybox-portable, so a hard
/// cap cannot be expressed with portable flags. The bound is `-T 15`
/// per attempt, the check is an explicit user action (Ctrl-C-able),
/// and this path only runs when curl is absent.
fn wget_args() -> [&'static str; 10] {
    [
        "-q",
        "-O",
        "-",
        "-T",
        "15",
        "--header",
        "Accept: application/vnd.github+json",
        "--header",
        "User-Agent: cosmostrix",
        GITHUB_API_URL,
    ]
}

fn run_tool(tool: &str, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new(tool).args(args).output()
}

/// Parse the curl response (body + trailing status line) and report.
fn check_curl_output(current_version: &str, output: &std::process::Output) -> Result<(), String> {
    if !output.status.success() {
        return Err(curl_failure(output.status.code().unwrap_or(-1)).to_string());
    }

    let raw = String::from_utf8(output.stdout.to_vec())
        .map_err(|_| "response was not valid UTF-8".to_string())?;
    let (body, status) = raw
        .rsplit_once('\n')
        .ok_or_else(|| "GitHub API response was malformed".to_string())?;
    let status = status.trim().parse::<u16>().unwrap_or(0);
    if status != 200 {
        return Err(http_failure(status).to_string());
    }

    report_update_status(current_version, body)
}

/// Compare `current_version` against the latest release tag in `body`
/// and print the user-facing update report.
fn report_update_status(current_version: &str, body: &str) -> Result<(), String> {
    let latest_tag = extract_tag_name(body)
        .ok_or_else(|| "could not parse latest release tag from GitHub response".to_string())?;
    let status = match compare_versions(current_version, &latest_tag) {
        UpdateStatus::UpToDate => "up to date",
        UpdateStatus::UpdateAvailable => "update available",
        UpdateStatus::CurrentIsNewer => "current is newer than latest release",
    };

    println_safe!("cosmostrix update check");
    println_safe!("Current: {}", normalize_version(current_version));
    println_safe!("Latest:  {}", normalize_version(&latest_tag));
    println_safe!("Status:  {status}");
    println_safe!("Source:  {RELEASES_URL}");

    Ok(())
}

/// Fetch the latest release tag and report the update status.
///
/// Fetcher strategy (NIGHT-hunter-7, owner hunt 2026-09-05): curl first —
/// its `--write-out` contract gives the exact HTTP status code. When the
/// OS has no curl on PATH (Alpine busybox, minimal containers, hardened
/// or older systems), fall back to wget instead of failing: `wget -q -O -`
/// only exits 0 on success-class responses, so the exit status carries
/// the failure class. When neither tool exists, the error is actionable
/// (names both tools and the manual releases URL) instead of a dead end.
pub(crate) fn check_update(current_version: &str) -> Result<(), String> {
    match run_tool("curl", &curl_args()) {
        Ok(output) => return check_curl_output(current_version, &output),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("failed to run curl: {e}"));
        }
        // curl not installed — fall through to the wget fallback.
        Err(_) => {}
    }

    match run_tool("wget", &wget_args()) {
        Ok(output) if output.status.success() => {
            let body = String::from_utf8(output.stdout)
                .map_err(|_| "response was not valid UTF-8".to_string())?;
            report_update_status(current_version, &body)
        }
        Ok(output) => Err(wget_failure(output.status.code().unwrap_or(-1)).to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(NO_FETCHER_MSG.to_string()),
        Err(e) => Err(format!("failed to run wget: {e}")),
    }
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

    /// NIGHT-hunter-7: the curl argv contract — silent, 15 s total-time
    /// cap, GitHub JSON accept + UA, and the trailing http_code line that
    /// `check_curl_output` parses. Any drift here breaks status parsing.
    #[test]
    fn curl_args_contract() {
        assert_eq!(
            curl_args(),
            [
                "--silent",
                "--max-time",
                "15",
                "--header",
                "Accept: application/vnd.github+json",
                "--header",
                "User-Agent: cosmostrix",
                "--write-out",
                "\n%{http_code}",
                CANONICAL_GITHUB_API_URL,
            ]
        );
    }

    /// NIGHT-hunter-7: the wget argv contract — busybox/GNU intersection
    /// (no retry flags, no server-response flag): status classification
    /// relies on the exit code, not a parsed status line.
    #[test]
    fn wget_args_contract() {
        let args = wget_args();
        assert_eq!(
            args,
            [
                "-q",
                "-O",
                "-",
                "-T",
                "15",
                "--header",
                "Accept: application/vnd.github+json",
                "--header",
                "User-Agent: cosmostrix",
                CANONICAL_GITHUB_API_URL,
            ]
        );
        // The fallback must not depend on GNU-only flags.
        for flag in args {
            assert!(
                !flag.starts_with("--tries")
                    && !flag.starts_with("--waitretry")
                    && !flag.starts_with("--server-response"),
                "wget argv contains a GNU-only flag: {flag}"
            );
        }
    }

    /// NIGHT-hunter-7: wget exit-code classification — server error vs
    /// generic network failure (busybox collapses to 1).
    #[test]
    fn wget_failure_classifies_exit_codes() {
        assert_eq!(wget_failure(8), "GitHub API returned an unexpected error");
        assert_eq!(wget_failure(1), "network request failed");
        assert_eq!(wget_failure(4), "network request failed");
    }

    /// NIGHT-hunter-7: the both-missing error must stay actionable —
    /// name the fallback tool AND the manual URL.
    #[test]
    fn no_fetcher_message_is_actionable() {
        assert!(CANONICAL_NO_FETCHER_MSG.contains("wget"));
        assert!(CANONICAL_NO_FETCHER_MSG.contains(CANONICAL_RELEASES_URL));
    }

    /// NIGHT-hunter-7: curl output parsing — trailing status line split,
    /// non-200 classification, and the up-to-date report path.
    #[test]
    fn curl_output_parses_status_and_reports() {
        // Simulated curl stdout: JSON body + trailing http_code line.
        let mut output = std::process::Command::new("true").output().unwrap();
        output.stdout = b"{\"tag_name\":\"v9.9.9\"}\n200".to_vec();
        assert!(check_curl_output("9.9.9", &output).is_ok());

        output.stdout = b"{\"message\":\"Not Found\"}\n404".to_vec();
        assert_eq!(
            check_curl_output("9.9.9", &output),
            Err("no latest GitHub release found for oxyzenQ/cosmostrix".to_string())
        );
    }
}
