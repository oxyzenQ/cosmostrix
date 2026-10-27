#!/bin/sh
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# PLATFORM: Linux guest only (PID 1 inside a KVM micro-VM initramfs).
#
# The cosmostrix supermassive-kernel sandbox init - PID 1 inside the
# micro-VM booted by .github/workflows/supermassive-kernel.yml (the
# CI lanes). Ported from the sibling zejtron's scripts/sandbox/
# sandbox-init.sh (night-improve-43) after that lane's live CI runs
# proved the shape: no systemd, this script owns every mount, it
# must NEVER exit without ending the VM, and the exit rides qemu's
# isa-debug-exit port device - reboot(2) poweroff silently no-ops
# in a direct-boot guest.
#
# Two lanes, one init. The FLOOR lane (the kernel-floor job): the
# kernel that booted must BE the documented floor kernel (2.6.27-*,
# the intrepid GA release; a wrong kernel riding the floor lane's
# name fails HERE, before any payload spends its budget). The BEST
# lane (the kernel-best job): the booted kernel is the report -
# moving-latest by construction, so the identity row states what
# booted and the payload's exit code is the verdict. The lane rides
# in as /opt/cosmostrix/.sandbox/lane (floor or best); a garbage
# lane is an assembly bug - fail loudly, never guess.
#
# The userland is the packer's business (the floor lane packs a
# busybox skeleton, the best lane packs an ubuntu:24.04 export);
# this init only needs /bin/sh (POSIX), mount, uname, dd, sleep and
# the /dev/console character device the initramfs carries as a
# static node (a docker-export userland cannot restore device nodes
# from tar, so the packer mknods them as root - see the workflow).
#
# Contract: SANDBOX-RESULT lines for the bring-up probes plus one
# final SANDBOX-VERDICT line (PASS, or FAIL with the reason), all on
# the serial console - the entrypoint greps these sentinels; the VM
# cannot relay exit codes through qemu, the sentinel lines ARE the
# relay. Pure ASCII on purpose: the sentinel contract is grep'ed.

set -u
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export TERM="${TERM:-dumb}"

FAILURES=0
LANE="floor"

note() {
	echo "SANDBOX-RESULT: $1 - $2"
	# The floor lane gates on bring-up; the best lane only reports -
	# there the payload's exit code is the verdict.
	if [ "$2" != "PASS" ] && [ "$LANE" = "floor" ]; then
		FAILURES=$((FAILURES + 1))
	fi
}

guest_exit() {
	# The deterministic VM exit, belts in order (the sibling's proven
	# order). (1) The console drain: the sentinel lines are the
	# contract, and qemu's isa-debug-exit device ends the process the
	# moment one byte lands on port 0xf4 - serial bytes still in
	# flight die with it. One quiet second lets the UART FIFO and the
	# host-side pipe run dry before the exit byte is written.
	# (2) The isa-debug-exit device: /dev/port is the root-writable
	# port-I/O window (seek = the port number), and the written value
	# 0 makes qemu exit with status 1 - an rc the entrypoint accepts
	# as the guest's deliberate exit. (3) The sysrq poweroff:
	# enabling sysrq first is LOAD-BEARING (a bare "echo o" silently
	# no-ops under the default sysrq mask); with sysrq enabled the
	# write reaches ACPI S5 where the kernel supports it, qemu exits
	# 0, also accepted. (4) The sleep loop (never spin the host's
	# budget - the outer timeout owns the last resort).
	echo 1 >/proc/sys/kernel/sysrq 2>/dev/null || true
	sync 2>/dev/null || true
	sleep 1 2>/dev/null || true
	printf '\x00' | dd of=/dev/port bs=1 seek=244 count=1 2>/dev/null || true
	echo o >/proc/sysrq-trigger 2>/dev/null || true
	while true; do sleep 60; done
}

Z=/opt/cosmostrix
if ! cd "$Z"; then
	# A PID 1 must never plain-exit (the guest would panic-wedge):
	# exit through the port device with the verdict already naming
	# the failure.
	echo "SANDBOX-VERDICT: FAIL (cannot cd $Z - initramfs assembly bug)"
	guest_exit
fi

# The kernel surfaces, before anything that needs them. devtmpfs is
# OPTIONAL on purpose: the floor lane's 2.6.27 guest predates the
# devtmpfs default and the packer already landed static /dev nodes,
# so a failed devtmpfs mount is a report, not a failure.
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs dev /dev 2>/dev/null || true
exec >/dev/console 2>&1
echo "SANDBOX: init on $(uname -r)"

# The lane file decides the probes' posture.
if [ -r "$Z/.sandbox/lane" ]; then
	LANE="$(cat "$Z/.sandbox/lane" 2>/dev/null)"
fi
LANE="${LANE:-floor}"
case "$LANE" in
floor | best) ;;
*)
	echo "SANDBOX-VERDICT: FAIL (unknown lane: $LANE - assembly bug)"
	guest_exit
	;;
esac

# Everything the battery needs: writable scratch (the payload's
# logs) and, on the best lane, devpts (the pty harness allocates
# real ptys). The floor lane's busybox userland has no pty harness;
# proc/sys suffice there.
if mount -t tmpfs tmp /tmp && mount -t tmpfs run /run; then
	note "tmpfs scratch (/tmp, /run)" PASS
else
	note "tmpfs scratch (/tmp, /run)" FAIL
fi
if [ "$LANE" = "best" ]; then
	if mkdir -p /dev/pts 2>/dev/null && mount -t devpts devpts /dev/pts; then
		note "devpts (the pty harness substrate)" PASS
	else
		note "devpts (the pty harness substrate)" FAIL
	fi
fi

# The lane's kernel contract. Floor lane: the kernel that booted
# must BE the documented floor kernel - 2.6.27-* (docs/
# SYSTEM_REQUIREMENTS.md "Practical minimum: Linux 2.6.27"); a newer
# kernel riding the floor lane's name (a resolver regression, a
# cached wrong vmlinuz) fails HERE, loudly, before the payload.
# Best lane: the release is REPORTED (moving-latest by construction
# - the resolver picks the newest mainline stable), so the row
# states what booted and never gates.
if [ "$LANE" = "floor" ]; then
	case "$(uname -r)" in
	2.6.27-*)
		note "kernel floor (release $(uname -r) is the intrepid GA 2.6.27 floor kernel)" PASS
		;;
	*)
		note "kernel floor (release $(uname -r) is NOT the 2.6.27 floor kernel - wrong kernel booted)" FAIL
		;;
	esac
else
	echo "SANDBOX-RESULT: kernel witness (release $(uname -r), moving-latest) - PASS"
fi

# The lean prelude: the binary runs and answers --version on this
# kernel, before any payload spends its budget.
if "$Z/cosmostrix" --version >/dev/console 2>&1; then
	note "cosmostrix --version (the binary runs on this kernel)" PASS
else
	note "cosmostrix --version (the binary runs on this kernel)" FAIL
fi

# The payload. Whatever the entrypoint packed at .sandbox/payload.sh;
# its exit code IS the verdict. Bring-up FAILURES above fail the run
# too on the floor lane (a guest that cannot host the lane has no
# payload verdict worth reporting) - on the best lane the payload is
# the verdict, bring-up only reported.
PAYLOAD="$Z/.sandbox/payload.sh"
if [ "$FAILURES" -ne 0 ]; then
	echo "SANDBOX-VERDICT: FAIL (bring-up probes failed - payload never ran)"
	guest_exit
fi
if [ -x "$PAYLOAD" ]; then
	echo "SANDBOX: running the packed payload..."
	sh "$PAYLOAD"
	rc=$?
else
	echo "SANDBOX-VERDICT: FAIL (no payload packed - assembly bug)"
	guest_exit
fi
if [ "$rc" -eq 0 ]; then
	echo "SANDBOX-VERDICT: PASS"
else
	echo "SANDBOX-VERDICT: FAIL (payload exit $rc)"
fi
guest_exit
