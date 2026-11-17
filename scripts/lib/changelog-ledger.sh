# shellcheck shell=bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# PLATFORM: UNIX-only (Linux, macOS, BSD). Sourced by
#   scripts/dev/version-to.sh (the writer entry) and
#   scripts/gates/check-release-ledger.sh (the read-only shape
#   gate) - not a standalone executable (no main, no argument
#   parsing; the entry scripts own both).
#
# COSMOSTRIX VERSION MODULE: changelog_ledger
# The CHANGELOG.md release-ledger machinery behind version-to.sh's
# bump lane. Three machine-checked laws (the zelynic NIGHT-bump-1
# port; the fatal class the owner hit on the v80.0.x line: the
# bump's version line rode below the unreleased section because
# nothing ever verified where a write landed):
#
#   1. Shape law - exactly one '## Released' and one '## Unreleased'
#      heading, Released first, the owner's pure shape inside
#      Released: the blank spacer under the heading, bare vX.Y.Z
#      version lines, nothing else. A Released block below
#      Unreleased relocates when it carries the pure shape only;
#      foreign content refuses - a ledger this module did not
#      create is not one it rewrites.
#   2. Semver law - a bump below the ledger's top refuses as the
#      stale-tree downgrade; --force records a deliberate
#      release-line reset.
#   3. Verification law - after any splice the file is re-parsed and
#      the line must sit inside '## Released' exactly once with
#      Released above Unreleased; any other outcome restores the
#      backup and fails the run. A write this module cannot verify
#      is a write it does not keep.
#
# The splice overwrites through cat (not mv) so CHANGELOG.md keeps
# its inode and its 644 permission bits. The writer entry
# (changelog_ledger_preflight) relies on the sourcing script's
# logging helpers (log_info/log_ok/log_warn/log_err) and exits 1 on
# any refused shape - a release event whose ledger cannot be
# verified must not proceed. The probe functions are pure and shared
# with the gate.

# ── Probes (pure: read-only, no logging, safe to source anywhere) ──────

# The '## Released' section's bare version lines, in file order.
# The ledger is parsed, never assumed.
ledger_versions() {
	local file="$1"
	awk '/^## Released$/{f=1;next} /^## Unreleased$/{f=0} f&&/^v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$/' "${file}"
}

# ledger_released_count FILE LINE - how many exact-line copies LINE
# carries inside the '## Released' section. Presence anywhere else
# in the file is invisible here on purpose: the callers compare this
# count against the whole-file count to detect strands.
ledger_released_count() {
	local file="$1" line="$2"
	awk -v line="${line}" '
		/^## Released$/ { f = 1; next }
		f && /^## Unreleased$/ { f = 0 }
		f && $0 == line { n++ }
		END { print n + 0 }
	' "${file}"
}

# ledger_order_ok FILE - '## Released' exactly once, '## Unreleased'
# exactly once, Released above Unreleased. The file's historical
# narrative headings (## v4.0.0 and friends) are outside this law.
ledger_order_ok() {
	local file="$1"
	local rcount ucount rline uline
	rcount="$(grep -c '^## Released$' "${file}" || true)"
	ucount="$(grep -c '^## Unreleased$' "${file}" || true)"
	if [ "${rcount}" -ne 1 ] || [ "${ucount}" -ne 1 ]; then
		return 1
	fi
	rline="$(grep -n '^## Released$' "${file}" | cut -d: -f1)"
	uline="$(grep -n '^## Unreleased$' "${file}" | cut -d: -f1)"
	[ "${rline}" -lt "${uline}" ]
}

# ledger_spacer_blank FILE - the line under the '## Released' heading
# is blank: the shape the splice relies on, refused rather than
# guessed around when absent.
ledger_spacer_blank() {
	local file="$1"
	[ -z "$(sed -n '/^## Released$/{n;p;}' "${file}")" ]
}

# ledger_impurities FILE - every line inside '## Released' that is
# neither blank nor a bare version line. Empty output means the
# section carries the owner's pure shape.
ledger_impurities() {
	local file="$1"
	awk '
		/^## Released$/ { f = 1; next }
		f && /^## Unreleased$/ { f = 0 }
		f && !/^$/ && !/^v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$/ { print }
	' "${file}"
}

# ver_lt A B - true when A sorts strictly below B in semver
# precedence order. Primary comparator: sort -V over the numeric
# X.Y.Z prefix. Tiebreak (same prefix): semver places a pre-release
# below its own stable (v100.0.10-night.1 sorts below v100.0.10),
# and two pre-release labels ride sort -V again on the suffix. Raw
# sort -V alone would order the suffix above the stable and refuse
# the stable cut of a recorded pre-release - the
# v100.0.10-night.1 -> v100.0.10 flow in this repo's own ledger -
# as a false downgrade.
ver_lt() {
	local a="${1#v}" b="${2#v}"
	local ap="${a%%-*}" bp="${b%%-*}"
	local as="" bs=""
	if [ "${a}" != "${ap}" ]; then as="${a#"${ap}"}"; fi
	if [ "${b}" != "${bp}" ]; then bs="${b#"${bp}"}"; fi
	local first
	first="$(printf '%s\n%s\n' "${ap}" "${bp}" | sort -V | head -1)"
	if [ "${ap}" != "${bp}" ]; then
		[ "${first}" = "${ap}" ]
		return
	fi
	# Same numeric prefix - the pre-release tiebreak. A stable line
	# sorts above any suffixed twin; two suffixed twins compare on
	# the suffix alone.
	if [ -z "${as}" ] || [ -z "${bs}" ]; then
		[ -n "${as}" ]
		return
	fi
	first="$(printf '%s\n%s\n' "${as#-}" "${bs#-}" | sort -V | head -1)"
	[ "${first}" = "${as#-}" ] && [ "${as#-}" != "${bs#-}" ]
}

# ── Heals and the writer entry (own the file, fail loudly) ─────────────

# ledger_relocate_below FILE RLINE ULINE - moves a '## Released'
# block that sits below '## Unreleased' back above it (shape law 1b,
# the block form of the incident class). Only the pure shape
# relocates: heading, blank spacers, bare version lines - nothing
# else. The result is re-verified; a failed relocation restores the
# file and fails the run.
ledger_relocate_below() {
	local file="$1" rline="$2" uline="$3"
	local bend bline blk bad tmp backup
	# The block ends where the next '## ' heading begins (or EOF).
	bline="$(tail -n "+${rline}" "${file}" | grep -nE '^## ' | sed -n '2p' | cut -d: -f1 || true)"
	if [ -n "${bline}" ]; then
		bend=$((rline + bline - 2))
	else
		bend="$(grep -c '' "${file}")"
	fi
	blk="$(sed -n "${rline},${bend}p" "${file}")"
	bad="$(printf '%s\n' "${blk}" | ledger_impurities /dev/stdin | head -3 || true)"
	if [ -n "${bad}" ]; then
		log_err "CHANGELOG.md carries a ## Released block below ## Unreleased AND foreign content inside it:"
		printf '%s\n' "${bad}" >&2
		log_err "A ledger this script did not create is not one it rewrites - repair the shape by hand, then re-run"
		exit 1
	fi
	tmp="$(mktemp)"
	backup="$(mktemp)"
	cp "${file}" "${backup}"
	awk -v blk="${blk}" -v uline="${uline}" -v rline="${rline}" -v bend="${bend}" '
		NR >= rline && NR <= bend { next }
		NR == uline {
			printf "%s\n\n", blk
			print
			next
		}
		{ print }
	' "${file}" >"${tmp}"
	cat "${tmp}" >"${file}"
	rm -f "${tmp}"
	if ! ledger_order_ok "${file}"; then
		cat "${backup}" >"${file}"
		rm -f "${backup}"
		log_err "The ## Released relocation did not verify (Released still below Unreleased) - the file was restored, nothing kept"
		exit 1
	fi
	rm -f "${backup}"
	log_ok "CHANGELOG.md ## Released relocated above ## Unreleased (the owner shape restored)"
}

# changelog_ledger_preflight VERSION FORCE - the whole ledger work,
# run as a pre-flight before any file is mutated: shape checks, the
# repairable heals, the semver gate, and the verified splice. The
# already-recorded case returns before any mutation, so re-runs stay
# idempotent and the entry script's early-exit path still heals
# through its own verification.
changelog_ledger_preflight() {
	local ver="v$1"
	local force="${2:-0}"

	local changelog="${REPO_ROOT}/CHANGELOG.md"
	if [ ! -f "${changelog}" ]; then
		log_warn "CHANGELOG.md not found - release line not recorded"
		return 0
	fi

	# Law 1a: exactly one of each heading. More than one is a shape
	# this module cannot parse and will not edit.
	local rcount ucount
	rcount="$(grep -c '^## Released$' "${changelog}" || true)"
	ucount="$(grep -c '^## Unreleased$' "${changelog}" || true)"
	if [ "${rcount}" -gt 1 ] || [ "${ucount}" -gt 1 ]; then
		log_err "CHANGELOG.md carries ${rcount} '## Released' heading(s) and ${ucount} '## Unreleased' heading(s); expected exactly one of each"
		log_err "Repair the headings by hand, then re-run the bump"
		exit 1
	fi

	# Law 1b: Released must precede Unreleased. The pure-block shape
	# below Unreleased relocates; anything richer refuses.
	if [ "${rcount}" -eq 1 ] && [ "${ucount}" -eq 1 ]; then
		local rline uline
		rline="$(grep -n '^## Released$' "${changelog}" | cut -d: -f1)"
		uline="$(grep -n '^## Unreleased$' "${changelog}" | cut -d: -f1)"
		if [ "${rline}" -gt "${uline}" ]; then
			ledger_relocate_below "${changelog}" "${rline}" "${uline}"
		fi
	fi

	# Law 1c: the owner's pure shape inside Released - blank spacers
	# and bare version lines, nothing else.
	if [ "${rcount}" -eq 1 ]; then
		local junk
		junk="$(ledger_impurities "${changelog}" | head -3 || true)"
		if [ -n "${junk}" ]; then
			log_err "CHANGELOG.md carries foreign content inside ## Released (the owner shape is blank spacers + bare version lines):"
			printf '%s\n' "${junk}" >&2
			log_err "A ledger this script did not create is not one it rewrites - repair the shape by hand, then re-run"
			exit 1
		fi
		if ! ledger_spacer_blank "${changelog}"; then
			log_err "CHANGELOG.md lost the blank spacer under ## Released (the owner shape is heading, blank line, version lines) - restore it, then re-run"
			exit 1
		fi
	fi

	# The version line never repeats - and a line stranded OUTSIDE
	# Released (the incident class) heals in place: the strays are
	# deleted now, the verified splice below writes the line where it
	# belongs.
	local total inside
	total="$(grep -cxF "${ver}" "${changelog}" || true)"
	inside="$(ledger_released_count "${changelog}" "${ver}")"
	if [ "${total}" -gt 0 ] && [ "${total}" -eq "${inside}" ]; then
		log_ok "CHANGELOG.md already records ${ver} inside ## Released"
		return 0
	fi

	# Everything below mutates the file - the backup is the run's
	# escape hatch: any refused law restores the file as found.
	local tmp backup
	backup="$(mktemp)"
	cp "${changelog}" "${backup}"

	if [ "${total}" -gt 0 ]; then
		tmp="$(mktemp)"
		grep -vxF "${ver}" "${changelog}" >"${tmp}" || true
		cat "${tmp}" >"${changelog}"
		rm -f "${tmp}"
		log_warn "CHANGELOG.md recorded ${ver} outside ## Released (${total} stray line(s), ${inside} inside) - strays deleted; the verified splice below records it properly"
	fi

	# Law 2: the semver gate - a bump below the ledger's top is the
	# stale-tree downgrade; --force records a deliberate release-line
	# reset.
	if [ -n "$(ledger_versions "${changelog}")" ]; then
		local top
		top="$(ledger_versions "${changelog}" | head -1)"
		if ver_lt "${ver}" "${top}"; then
			if [ "${force}" != "1" ]; then
				cat "${backup}" >"${changelog}"
				rm -f "${backup}"
				log_err "${ver} is not a step up from the ledger's top (${top}) - a stale-tree downgrade, refused"
				log_err "A deliberate release-line reset is recorded with: ./scripts/dev/version-to.sh $1 --force"
				exit 1
			fi
			log_warn "Release-line reset recorded (--force): ${ver} takes the ledger's top position while sorting below ${top}"
		fi
	fi

	# Law 3 (the splice): write, then VERIFY - the version line must
	# sit inside ## Released exactly once with Released above
	# Unreleased; any other outcome restores the backup and fails the
	# run. A write this script cannot verify is a write it does not
	# keep.
	tmp="$(mktemp)"
	local spliced_where
	if [ "${rcount}" -eq 1 ]; then
		# The section exists with its verified spacer - the new version
		# splices in under it, above the current top line (newest
		# first). The lookahead keeps the blank spacer between the
		# ledger and the next heading when the section was empty.
		awk -v ver="${ver}" '
			/^## Released$/ && !done {
				print
				getline spacer
				print spacer
				print ver
				if ((getline nxt) > 0) {
					if (nxt != "") {
						print ""
					}
					print nxt
				}
				done = 1
				next
			}
			{ print }
		' "${changelog}" >"${tmp}"
		spliced_where="released"
	elif [ "${ucount}" -eq 1 ]; then
		# No section yet - create it above ## Unreleased (the
		# unreleased work keeps its own section below the ledger).
		awk -v ver="${ver}" '
			/^## Unreleased$/ && !done {
				print "## Released"
				print ""
				print ver
				print ""
				done = 1
			}
			{ print }
		' "${changelog}" >"${tmp}"
		spliced_where="created"
	else
		rm -f "${tmp}" "${backup}"
		log_warn "CHANGELOG.md has no ## Unreleased anchor - release line not recorded"
		return 0
	fi
	cat "${tmp}" >"${changelog}"
	rm -f "${tmp}"

	total="$(grep -cxF "${ver}" "${changelog}" || true)"
	inside="$(ledger_released_count "${changelog}" "${ver}")"
	if [ "${total}" -ne 1 ] || [ "${inside}" -ne 1 ] || ! ledger_order_ok "${changelog}"; then
		cat "${backup}" >"${changelog}"
		rm -f "${backup}"
		log_err "The ledger write did not verify (${ver} must sit inside ## Released exactly once, with Released above Unreleased) - CHANGELOG.md was restored, nothing kept"
		exit 1
	fi
	rm -f "${backup}"
	if [ "${spliced_where}" = "created" ]; then
		log_ok "CHANGELOG.md recorded ${ver} (## Released section created above ## Unreleased, verified in place)"
	else
		log_ok "CHANGELOG.md recorded ${ver} (## Released, newest first, verified in place)"
	fi
	return 0
}
