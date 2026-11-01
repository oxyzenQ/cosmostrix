// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! `write_config_atomic` error-path hygiene tests.
//!
//! The function's doc promises "best-effort cleanup on error": a failed
//! write, sync, or rename must not strand the `<target>.tmp.<pid>` temp
//! file in the config directory. The rename-failure test below pins that
//! contract — the target path is an existing directory, so `rename(2)`
//! fails (EISDIR) after the temp file was already created and written.
//! Before the fix, the early return left the temp on disk.

#![cfg(test)]

use std::fs;

#[test]
fn rename_failure_does_not_strand_temp_file() {
    let dir = std::env::temp_dir().join(format!("cx_cfg_io_{}_{}", std::process::id(), line!()));
    fs::create_dir_all(&dir).expect("create scratch dir");
    // Target is an existing DIRECTORY: creating it as the rename destination
    // makes rename(2) fail after the temp was written (file -> dir is EISDIR).
    let target = dir.join("config.toml");
    fs::create_dir_all(&target).expect("create target dir");

    let result = crate::config_io::write_config_atomic(
        target.to_str().expect("utf-8 scratch path"),
        "# example config\n",
    );
    assert!(result.is_err(), "rename onto a directory must fail");

    let stranded: Vec<_> = fs::read_dir(&dir)
        .expect("read scratch dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
        .collect();
    assert!(
        stranded.is_empty(),
        "failed rename must not strand temp files: {:?}",
        stranded.iter().map(|e| e.file_name()).collect::<Vec<_>>()
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn success_path_writes_target_and_leaves_no_temp() {
    let dir = std::env::temp_dir().join(format!("cx_cfg_io_{}_{}", std::process::id(), line!()));
    fs::create_dir_all(&dir).expect("create scratch dir");
    let target = dir.join("config.toml");
    let target_str = target.to_str().expect("utf-8 scratch path");

    crate::config_io::write_config_atomic(target_str, "key = 1\n").expect("atomic write");

    assert_eq!(
        fs::read_to_string(&target).expect("read target"),
        "key = 1\n"
    );
    let temps: Vec<_> = fs::read_dir(&dir)
        .expect("read scratch dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
        .collect();
    assert!(temps.is_empty(), "success must leave no temp files");

    let _ = fs::remove_dir_all(&dir);
}
