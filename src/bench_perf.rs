// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Microarchitectural perf counters (Linux x86 only).
//!
//! Phase 4 of DeepSeek benchmark restructuring plan.
//!
//! Uses `perf_event_open` syscall to read hardware counters:
//! CPU cycles, instructions, branch misses, branch instructions.
//! Falls back to "not available" on non-Linux or when perf is unavailable.

#[cfg(target_os = "linux")]
mod linux {
    use super::PerfRaw;

    const PERF_TYPE_HARDWARE: u32 = 0;
    const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
    const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
    const PERF_COUNT_HW_BRANCH_INSTRUCTIONS: u64 = 4;
    const PERF_COUNT_HW_BRANCH_MISSES: u64 = 5;

    // perf_event_attr struct (simplified — only fields we need)
    #[repr(C)]
    struct PerfEventAttr {
        type_: u32,
        size: u32,
        config: u64,
        sample_period_or_freq: u64,
        sample_type: u64,
        read_format: u64,
        flags: u64,
        wakeup_events_or_watermark: u32,
        bp_type: u32,
        bp_addr_or_config1: u64,
        bp_len_or_config2: u64,
        branch_sample_type: u64,
        sample_regs_user: u64,
        sample_stack_user: u32,
        clockid: i32,
        sample_regs_intr: u64,
        aux_watermark: u32,
        sample_max_stack: u16,
        reserved2: u16,
    }

    impl Default for PerfEventAttr {
        fn default() -> Self {
            Self {
                type_: PERF_TYPE_HARDWARE,
                size: std::mem::size_of::<PerfEventAttr>() as u32,
                config: 0,
                sample_period_or_freq: 0,
                sample_type: 0,
                read_format: 0,
                flags: 0,
                wakeup_events_or_watermark: 0,
                bp_type: 0,
                bp_addr_or_config1: 0,
                bp_len_or_config2: 0,
                branch_sample_type: 0,
                sample_regs_user: 0,
                sample_stack_user: 0,
                clockid: 0,
                sample_regs_intr: 0,
                aux_watermark: 0,
                sample_max_stack: 0,
                reserved2: 0,
            }
        }
    }

    extern "C" {
        fn syscall(num: libc::c_long, ...) -> libc::c_long;
        fn close(fd: libc::c_int) -> libc::c_int;
    }

    const SYS_PERF_EVENT_OPEN: libc::c_long = 298;

    fn open_counter(config: u64) -> Option<i32> {
        unsafe {
            let attr = PerfEventAttr {
                config,
                ..Default::default()
            };
            // pid=0 (this process), cpu=0 (specific core — needed for perf_event_open),
            // group_fd=-1, flags=0
            let fd = syscall(
                SYS_PERF_EVENT_OPEN,
                &attr as *const PerfEventAttr as *mut PerfEventAttr,
                0i32,  // pid=0: measure this process
                0i32,  // cpu=0: specific core (perf requires this, not -1)
                -1i32, // group_fd=-1: standalone
                0u64,  // flags=0
            );
            if fd < 0 {
                None
            } else {
                let fd = fd as i32;
                // PERF_EVENT_IOC_ENABLE = _IO('$', 0) = 0x2400
                // PERF_EVENT_IOC_DISABLE = _IO('$', 1) = 0x2401
                // PERF_EVENT_IOC_RESET = _IO('$', 3) = 0x2403
                // Enable the counter (it starts counting immediately)
                libc::ioctl(fd, 0x2400u64 as _); // ENABLE
                Some(fd)
            }
        }
    }

    fn read_counter(fd: i32) -> u64 {
        if fd < 0 {
            return 0;
        }
        let mut value: u64 = 0;
        let ret = unsafe { libc::read(fd, &mut value as *mut u64 as *mut libc::c_void, 8) };
        if ret != 8 {
            return 0;
        }
        value
    }

    fn close_counter(fd: i32) {
        if fd >= 0 {
            unsafe {
                close(fd);
            }
        }
    }

    pub struct PerfCounters {
        cycles_fd: i32,
        instructions_fd: i32,
        branches_fd: i32,
        misspredicts_fd: i32,
        pub available: bool,
    }

    impl PerfCounters {
        pub fn new() -> Self {
            let cycles_fd = open_counter(PERF_COUNT_HW_CPU_CYCLES).unwrap_or(-1);
            let instructions_fd = open_counter(PERF_COUNT_HW_INSTRUCTIONS).unwrap_or(-1);
            let branches_fd = open_counter(PERF_COUNT_HW_BRANCH_INSTRUCTIONS).unwrap_or(-1);
            let misspredicts_fd = open_counter(PERF_COUNT_HW_BRANCH_MISSES).unwrap_or(-1);

            let available = cycles_fd >= 0 && instructions_fd >= 0;

            Self {
                cycles_fd,
                instructions_fd,
                branches_fd,
                misspredicts_fd,
                available,
            }
        }

        pub fn read(&self) -> PerfRaw {
            if !self.available {
                return PerfRaw::default();
            }
            PerfRaw {
                cycles: read_counter(self.cycles_fd),
                instructions: read_counter(self.instructions_fd),
                branch_instructions: read_counter(self.branches_fd),
                branch_misses: read_counter(self.misspredicts_fd),
                available: true,
            }
        }
    }

    impl Drop for PerfCounters {
        fn drop(&mut self) {
            close_counter(self.cycles_fd);
            close_counter(self.instructions_fd);
            close_counter(self.branches_fd);
            close_counter(self.misspredicts_fd);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PerfRaw {
    pub cycles: u64,
    pub instructions: u64,
    pub branch_instructions: u64,
    pub branch_misses: u64,
    pub available: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PerfMetrics {
    pub available: bool,
    pub cycles: u64,
    pub instructions: u64,
    pub instructions_per_cycle: f64,
    pub branch_instructions: u64,
    pub branch_misses: u64,
    pub branch_mispredict_rate: f64,
}

impl PerfRaw {
    pub fn delta(&self, before: &Self) -> PerfMetrics {
        if !self.available || !before.available {
            return PerfMetrics::default();
        }

        let cycles = self.cycles.saturating_sub(before.cycles);
        let instructions = self.instructions.saturating_sub(before.instructions);
        let branches = self
            .branch_instructions
            .saturating_sub(before.branch_instructions);
        let misses = self.branch_misses.saturating_sub(before.branch_misses);

        let ipc = if cycles > 0 {
            instructions as f64 / cycles as f64
        } else {
            0.0
        };
        let mispred_rate = if branches > 0 {
            (misses as f64 / branches as f64) * 100.0
        } else {
            0.0
        };

        PerfMetrics {
            available: true,
            cycles,
            instructions,
            instructions_per_cycle: ipc,
            branch_instructions: branches,
            branch_misses: misses,
            branch_mispredict_rate: mispred_rate,
        }
    }
}

/// Open perf counters for measurement. Returns None on non-Linux.
pub fn open_counters() -> Option<PerfCounterHandle> {
    #[cfg(target_os = "linux")]
    {
        let counters = linux::PerfCounters::new();
        if counters.available {
            Some(PerfCounterHandle {
                inner: Some(counters),
            })
        } else {
            None
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

pub struct PerfCounterHandle {
    #[cfg(target_os = "linux")]
    inner: Option<linux::PerfCounters>,
    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    inner: Option<()>,
}

impl PerfCounterHandle {
    pub fn read(&self) -> PerfRaw {
        #[cfg(target_os = "linux")]
        {
            if let Some(ref inner) = self.inner {
                return inner.read();
            }
        }
        PerfRaw::default()
    }
}
