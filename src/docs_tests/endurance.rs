// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Resource monitor, endurance docs, and gitignore guards.

#[test]
fn monitor_script_has_spdx_and_is_executable() {
    let script = std::fs::read_to_string("scripts/monitor-cosmostrix.sh")
        .expect("monitor script must exist");
    assert!(script.contains("SPDX-License-Identifier: MIT"));
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata("scripts/monitor-cosmostrix.sh")
        .unwrap()
        .permissions()
        .mode();
    assert!(mode & 0o111 != 0);
}

#[test]
fn endurance_docs_mention_monitor_and_gitignored() {
    let docs = include_str!("../../docs/ENDURANCE.md");
    assert!(docs.contains("scripts/monitor-cosmostrix.sh"));
    let lower = docs.to_lowercase();
    assert!(lower.contains("gitignored") && lower.contains("local artifact"));
}

#[test]
fn gitignore_contains_resource_log_rules() {
    let g = include_str!("../../.gitignore");
    assert!(
        g.contains("/logs/") && g.contains("/benchmark/logs/") && g.contains("*-resource-*.csv")
    );
}
