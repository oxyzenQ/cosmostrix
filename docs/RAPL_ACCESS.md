# RAPL Energy Access Guide

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> How to enable ENERGY metrics in cosmostrix benchmark without running as root.

## Why RAPL Requires Root

The RAPL (Running Average Power Limit) energy counters are exposed by the
Linux kernel at `/sys/class/powercap/intel-rapl:*/energy_uj`. These files
are owned by `root:root` with mode `0400` (readable only by root) on most
distributions. This is a security measure — energy data could theoretically
be used for side-channel attacks.

## Method 1: Manual Permission Change (Temporary)

Simplest method, but resets on reboot:

```bash
# Find all energy_uj files and make them world-readable
sudo chmod a+r /sys/class/powercap/intel-rapl:*/energy_uj

# Verify
cat /sys/class/powercap/intel-rapl:0/energy_uj
# Should print a number like: 12345678

# Now run cosmostrix benchmark — ENERGY section will show data
cosmostrix --benchmark --screen-size 120x40 --bench-duration 5s
```

**Note**: Permissions reset on reboot because `sysfs` is a virtual filesystem.

## Method 2: udev Rule (Permanent, Recommended)

Create a udev rule that sets permissions automatically on boot:

```bash
# Create the rule file
sudo tee /etc/udev/rules.d/99-rapl.rules << 'EOF'
SUBSYSTEM=="powercap", KERNEL=="intel-rapl:*", MODE="0444"
EOF

# Reload udev rules
sudo udevadm control --reload-rules
sudo udevadm trigger

# Verify
cat /sys/class/powercap/intel-rapl:0/energy_uj
```

This is the recommended method — it persists across reboots and only
grants read access to powercap files (no write, no other system files).

## Method 3: setcap (Per-Binary)

Grant the cosmostrix binary the `cap_dac_read_search` capability, which
allows it to read any file regardless of permissions:

```bash
# Find the cosmostrix binary
which cosmostrix
# e.g., /usr/bin/cosmostrix

# Grant read capability
sudo setcap cap_dac_read_search=+ep /usr/bin/cosmostrix

# Verify
getcap /usr/bin/cosmostrix
# Should print: /usr/bin/cosmostrix cap_dac_read_search=ep

# Now cosmostrix can read RAPL without sudo
cosmostrix --benchmark --screen-size 120x40 --bench-duration 5s
```

**Security warning**: `cap_dac_read_search` allows the binary to read ANY
file on the system, including `/etc/shadow`. Only use this method if you
trust the cosmostrix binary and your system security. Reinstalling or
updating cosmostrix via package manager may remove the capability —
re-apply after updates.

## Troubleshooting

### "Module amd_energy not found"

AMD CPUs use the `intel-rapl` kernel interface (naming legacy). You do NOT
need the `amd_energy` module. The `intel-rapl` files at
`/sys/class/powercap/intel-rapl:0/energy_uj` work for both Intel and AMD.

### No files in /sys/class/powercap/

If the directory is empty or doesn't exist:

1. Check kernel config: `zcat /proc/config.gz | grep RAPL`
   - `CONFIG_POWERCAP=y` and `CONFIG_INTEL_RAPL=y` (or =m) are needed
2. Load the module: `sudo modprobe intel_rapl`
3. Some VMs (KVM, VirtualBox) don't expose RAPL — run on bare metal

### ENERGY still shows "not available" after setup

Check:
```bash
ls -la /sys/class/powercap/intel-rapl:0/energy_uj
cat /sys/class/powercap/intel-rapl:0/energy_uj
```

If the file exists and is readable, cosmostrix should detect it automatically.
The benchmark reads it at start and end of the run, computing the delta.

## Verification

After setup, run:
```bash
cosmostrix --benchmark --screen-size 120x40 --bench-duration 5s
```

The ENERGY section should show:
```
ENERGY
  status: available (RAPL)
  packages: 1
  total_energy: XX.XX J
  avg_power: XX.XX W
  energy_per_frame: X.X µJ
  energy_per_cell: X.X nJ
```
