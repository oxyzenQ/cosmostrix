# shellcheck shell=bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/dev/version-to.sh - not a standalone executable (no
#   main, no argument parsing; the entry script owns both).
#
# COSMOSTRIX VERSION MODULE: ridealong
# The adaptive crate ride-along lane behind version-to.sh's bump
# (night-cafe-mclaren-1, the generalization of the night-improve-9
# pgo-runner lane): every [package] manifest under the repo root
# other than the primary Cargo.toml - cosmostrix-fuzz, pgo-runner,
# and any crate the owner adds tomorrow, wherever it lives - rides
# the root version. Discovery is dynamic on purpose: a hardcoded
# path list is the exact stale-data class this repo keeps hunting
# down, so a new Cargo.toml is adopted by the next bump with no
# script edit, and its drift is caught by --check with no script
# edit either.
#
# The manifest parse is an awk state machine over the [package]
# table, so a [[bin]] name or a dependency table never leaks into
# the crate identity. Lock entries move name-anchored, one entry -
# dependency versions never do (they are cargo update's job, not a
# version bump's). The law is strict and uniform: a ride-along
# crate at any version other than the root's fails --check, and
# the next version-to.sh run adopts it. There is no birth-version
# courtesy - a drifting 0.0.0 or 0.1.0 label shipping beside a
# v100 binary is the provenance hole this lane exists to close
# (the night-improve-9 wording, now repo-wide). Every probe probes
# honestly: a missing file warns and writes nothing.

# ── Probes (pure: read-only, no logging, safe to source anywhere) ──────

# ridealong_pkg_field FILE FIELD - the FIELD ("name" or "version")
# value from the [package] table of a Cargo.toml manifest, or an
# empty string when the manifest carries no such line. An awk state
# machine: inpkg is true only between [package] and the next table
# header, so a [[bin]] name or a dependency's own version line
# never leaks into the crate identity.
ridealong_pkg_field() {
	local file="$1" field="$2"
	awk -v field="${field}" '
		/^\[/ { inpkg = ($0 == "[package]") }
		inpkg && index($0, field " = \"") == 1 {
			val = substr($0, length(field) + 5)
			sub(/"$/, "", val)
			print val
			exit
		}
	' "${file}"
}

# ridealong_manifests - every ride-along manifest under the repo
# root, absolute paths, one per line, sorted. The root Cargo.toml
# is excluded (it is the bump's primary, not a passenger); the
# noise directories are pruned (build artifacts, vendored third
# party, node modules). A manifest qualifies only when the awk
# parse yields both a name and a version, so a stub or a template
# fragment without a real [package] never enters the lane.
ridealong_manifests() {
	find "${REPO_ROOT}" \
		\( -name .git -o -name target -o -name vendor -o -name node_modules \) -prune -o \
		-type f -name Cargo.toml -print |
		while IFS= read -r manifest; do
			[ "${manifest}" = "${CARGO_TOML}" ] && continue
			if [ -n "$(ridealong_pkg_field "${manifest}" name)" ] &&
				[ -n "$(ridealong_pkg_field "${manifest}" version)" ]; then
				printf '%s\n' "${manifest}"
			fi
		done | sort
}

# ── The writer lane (mutates, verifies every write in place) ───────────

# update_ridealong_crates NEW_VER - adopt every discovered ride-along
# crate onto the root version: the manifest line is rewritten and
# re-parsed (a write that does not verify fails the bump), then the
# sibling lock entry moves name-anchored, one entry. A manifest
# failure is fatal; a lock failure warns with the manual command -
# the verifier below owns the final verdict, mirroring the root
# Cargo.lock semantics. A missing lock or a lock without the
# crate's entry warns and writes nothing (honest probe: a crate
# that has never been locked has no entry to move).
update_ridealong_crates() {
	local new_ver="$1"
	local manifest crate crate_ver rel lock lock_ver got

	while IFS= read -r manifest; do
		[ -z "${manifest}" ] && continue
		crate="$(ridealong_pkg_field "${manifest}" name)"
		crate_ver="$(ridealong_pkg_field "${manifest}" version)"
		rel="${manifest#"${REPO_ROOT}"/}"
		if [ -z "${crate}" ] || [ -z "${crate_ver}" ]; then
			log_warn "Ride-along manifest without a parseable [package] name/version - skipped: ${rel}"
			continue
		fi
		if [ "${crate_ver}" = "${new_ver}" ]; then
			log_ok "${rel} already at ${new_ver} (ride-along lane)"
		else
			log_info "Updating ${rel}: ${crate_ver} -> ${new_ver} (${crate} rides the root version)"
			sed -i -E "s|^version = \"${crate_ver}\"|version = \"${new_ver}\"|" "${manifest}"
			got="$(ridealong_pkg_field "${manifest}" version)"
			if [ "${got}" != "${new_ver}" ]; then
				log_err "${rel} version update failed. Expected: ${new_ver}, Got: ${got}"
				exit 1
			fi
			log_ok "${rel} updated: version = \"${new_ver}\""
		fi
		lock="${manifest%/Cargo.toml}/Cargo.lock"
		if [ ! -f "${lock}" ]; then
			log_warn "${crate} has no Cargo.lock - lock update skipped (nothing written)"
			continue
		fi
		sed -i -E "/^name = \"${crate}\"$/{n;s|^version = \"${crate_ver}\"|version = \"${new_ver}\"|;}" "${lock}"
		lock_ver="$(grep -A1 "^name = \"${crate}\"$" "${lock}" 2>/dev/null |
			grep '^version = "' | head -1 | sed -E 's/^version = "(.+)"/\1/' || true)"
		if [ -z "${lock_ver}" ]; then
			log_warn "No ${crate} entry in ${lock#"${REPO_ROOT}"/} - lock skipped (cargo owns the entry's birth)"
			continue
		fi
		if [ "${lock_ver}" = "${new_ver}" ]; then
			log_ok "${lock#"${REPO_ROOT}"/} updated: ${crate} version = ${new_ver}"
		else
			log_err "${lock#"${REPO_ROOT}"/} update failed. Expected: ${new_ver}, Got: ${lock_ver}"
			log_err "You may need to run 'cargo update --manifest-path ${rel} -p ${crate}' manually"
		fi
	done < <(ridealong_manifests)
}

# ── The verifier lane (read-only, returns the error count) ─────────────

# verify_ridealong_crates EXPECTED_VER - every discovered ride-along
# manifest must carry the root version and its lock must agree with
# the manifest. Each failed check logs its own error and the total
# is returned as the exit status (the caller adds it to its error
# tally with `|| errs=$?`, safe under set -e). A lock that exists
# without the crate's entry is not an error: the crate has never
# been locked and cargo owns that birth.
verify_ridealong_crates() {
	local expected_ver="$1"
	local errors=0
	local manifest crate crate_ver rel lock lock_ver

	while IFS= read -r manifest; do
		[ -z "${manifest}" ] && continue
		crate="$(ridealong_pkg_field "${manifest}" name)"
		crate_ver="$(ridealong_pkg_field "${manifest}" version)"
		rel="${manifest#"${REPO_ROOT}"/}"
		if [ -z "${crate_ver}" ]; then
			log_err "Ride-along manifest without a parseable [package] version: ${rel}"
			errors=$((errors + 1))
			continue
		fi
		if [ "${crate_ver}" = "${expected_ver}" ]; then
			log_ok "${rel}: version = \"${expected_ver}\" (rides the root version)"
		else
			log_err "${rel}: expected ${expected_ver} (root ride-along), got ${crate_ver}"
			log_err "  Run './scripts/dev/version-to.sh v${expected_ver}' to adopt it"
			errors=$((errors + 1))
		fi
		lock="${manifest%/Cargo.toml}/Cargo.lock"
		if [ ! -f "${lock}" ]; then
			continue
		fi
		lock_ver="$(grep -A1 "^name = \"${crate}\"$" "${lock}" 2>/dev/null |
			grep '^version = "' | head -1 | sed -E 's/^version = "(.+)"/\1/' || true)"
		if [ -z "${lock_ver}" ]; then
			log_info "No ${crate} entry in ${lock#"${REPO_ROOT}"/} - lock agreement not checkable"
			continue
		fi
		if [ "${lock_ver}" = "${crate_ver}" ]; then
			log_ok "${lock#"${REPO_ROOT}"/}: ${crate} version = ${lock_ver} (agrees with the manifest)"
		else
			log_err "${lock#"${REPO_ROOT}"/}: expected ${crate_ver} (manifest ride-along), got ${lock_ver}"
			errors=$((errors + 1))
		fi
	done < <(ridealong_manifests)
	return "${errors}"
}
