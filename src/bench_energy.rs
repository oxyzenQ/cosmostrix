// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! RAPL energy measurement (Linux only).
//!
//! Phase 3 of DeepSeek benchmark restructuring plan.
//!
//! Reads `/sys/class/powercap/intel-rapl:*/energy_uj` for Intel CPUs
//! or `/sys/class/powercap/amd-rapl:*/energy_uj` for AMD CPUs.
//! Falls back to "not available" on non-Linux or when powercap is absent.

use std::fs;

#[derive(Debug, Clone, Default)]
pub struct EnergySnapshot {
    pub total_energy_uj: u64,
    pub package_count: u32,
    pub available: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EnergyMetrics {
    pub available: bool,
    pub total_energy_joules: f64,
    pub avg_power_watts: f64,
    pub energy_per_frame_uj: f64,
    pub energy_per_cell_nj: f64,
    pub package_count: u32,
}

impl EnergySnapshot {
    /// Read current RAPL energy from all packages.
    pub fn now() -> Self {
        let mut total_uj: u64 = 0;
        let mut pkg_count: u32 = 0;
        let mut found = false;

        // Scan /sys/class/powercap/ for any *-rapl:* entries with energy_uj
        // AMD CPUs use the intel-rapl interface (kernel naming legacy)
        if let Ok(entries) = fs::read_dir("/sys/class/powercap") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Match intel-rapl:0, amd-rapl:0, etc. (top-level packages only,
                // not sub-domains like intel-rapl:0:0)
                if name_str.contains("-rapl:") && !name_str.contains("-rapl:0:") {
                    let energy_path = entry.path().join("energy_uj");
                    if let Ok(energy_str) = fs::read_to_string(&energy_path) {
                        if let Ok(uj) = energy_str.trim().parse::<u64>() {
                            total_uj = total_uj.saturating_add(uj);
                            pkg_count += 1;
                            found = true;
                        }
                    }
                }
            }
        }

        Self {
            total_energy_uj: total_uj,
            package_count: pkg_count,
            available: found,
        }
    }

    /// Compute delta between two snapshots.
    pub fn delta(
        &self,
        before: &Self,
        elapsed_secs: f64,
        total_frames: u64,
        total_cells: u64,
    ) -> EnergyMetrics {
        if !self.available || !before.available {
            return EnergyMetrics::default();
        }

        // Handle counter wraparound (RAPL counters can wrap)
        let energy_delta_uj = if self.total_energy_uj >= before.total_energy_uj {
            self.total_energy_uj - before.total_energy_uj
        } else {
            // Wrapped — assume 64-bit counter, rare
            u64::MAX - before.total_energy_uj + self.total_energy_uj
        };

        let energy_joules = energy_delta_uj as f64 / 1_000_000.0;
        let power_watts = if elapsed_secs > 0.0 {
            energy_joules / elapsed_secs
        } else {
            0.0
        };
        let energy_per_frame = if total_frames > 0 {
            energy_delta_uj as f64 / total_frames as f64
        } else {
            0.0
        };
        let energy_per_cell = if total_cells > 0 {
            (energy_delta_uj as f64 * 1000.0) / total_cells as f64
        } else {
            0.0
        };

        EnergyMetrics {
            available: true,
            total_energy_joules: energy_joules,
            avg_power_watts: power_watts,
            energy_per_frame_uj: energy_per_frame,
            energy_per_cell_nj: energy_per_cell,
            package_count: self.package_count,
        }
    }
}
