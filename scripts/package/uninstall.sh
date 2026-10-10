#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky (oxyzenQ)
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses `sudo`, `rm -rf`, `install`, bash arrays — all require a POSIX-ish
#   environment. Will not run on Windows (Git Bash, Cygwin, WSL ok).
# ─────────────────────────────────────────────────────────────────────────────
#
# Uninstall cosmostrix: binary + config directories.
# --all (default) removes binary AND config. --purge is kept as a synonym
# for backward compatibility. Use --keep-config to preserve config.
#
# Auto-detects and removes the binary from:
#   /usr/bin/, ~/.local/bin/
# Sudo is used ONLY for system paths. Run WITHOUT sudo.

set -euo pipefail

PROJECT_NAME="cosmostrix"

usage() {
	cat <<EOF
Usage: $0 [--system|--user|--all] [--keep-config] [--purge]

  (default)  Remove binary AND config from all locations.
  --system   Remove only from /usr/bin (uses sudo).
             Also removes config files in /etc/${PROJECT_NAME}/.
  --user     Remove only from ~/.local/bin (no sudo).
             Also removes config files in ~/.config/${PROJECT_NAME}/.
  --all      Remove binary + config from all locations (default behavior).
  --keep-config  Preserve config files (only remove binary).
  --purge    Alias for --all (backward compatibility).

Symlink-safe: only known config files (config.toml, config.new.toml) are
removed. The cosmostrix directory itself is preserved if it is a symlink
or contains other files. Never rm -rf.

Sudo is used only for system paths. Run WITHOUT sudo.
EOF
}

MODE="--all"
KEEP_CONFIG=0
while [[ $# -gt 0 ]]; do
	case "$1" in
	--system)
		MODE="--system"
		shift
		;;
	--user)
		MODE="--user"
		shift
		;;
	--all)
		MODE="--all"
		shift
		;;
	--keep-config)
		KEEP_CONFIG=1
		shift
		;;
	--purge)
		MODE="--all"
		shift
		;; # backward compat: same as --all
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "error: unknown argument: $1" >&2
		usage
		exit 2
		;;
	esac
done

SYSTEM_PATHS=(/usr/bin)
USER_PATH="${HOME}/.local/bin"
removed=0

remove_at() {
	local target="$1"
	local need_sudo="$2"
	if [[ -f "${target}" ]]; then
		if [[ "${need_sudo}" == "yes" ]]; then
			sudo rm -f "${target}"
		else
			rm -f "${target}"
		fi
		echo "   removed: ${target}"
		removed=$((removed + 1))
	fi
}

# Symlink-safe directory cleanup: remove known config files inside
# the directory, then try to rmdir the parent (only succeeds if empty).
# NEVER rm -rf the directory itself — the owner may have symlinked it
# (e.g., ~/.config/cosmostrix → /mnt/dotfiles/cosmostrix), and rm -rf
# would follow the symlink and destroy the target tree.
remove_config_dir() {
	local target="$1"
	local need_sudo="$2"
	if [[ -d "${target}" ]]; then
		# Remove known config files only (not arbitrary contents).
		local files=("config.toml" "config.new.toml" ".install_tmp_default.toml")
		for f in "${files[@]}"; do
			local fp="${target}/${f}"
			if [[ -f "${fp}" ]] || [[ -L "${fp}" ]]; then
				if [[ "${need_sudo}" == "yes" ]]; then
					sudo rm -f "${fp}"
				else
					rm -f "${fp}"
				fi
				echo "   removed: ${fp}"
				removed=$((removed + 1))
			fi
		done
		# Try to remove the directory if now empty (rmdir fails if not
		# empty, or if it's a symlink — both are safe outcomes).
		if [[ "${need_sudo}" == "yes" ]]; then
			sudo rmdir "${target}" 2>/dev/null || true
		else
			rmdir "${target}" 2>/dev/null || true
		fi
		# Report directory cleanup (whether or not rmdir succeeded).
		if [[ ! -d "${target}" ]]; then
			echo "   removed: ${target}/"
		else
			echo "   cleaned: ${target}/ (directory preserved, not empty or is symlink)"
		fi
	fi
}

echo ">> Uninstalling ${PROJECT_NAME}"

case "${MODE}" in
--system)
	for p in "${SYSTEM_PATHS[@]}"; do
		remove_at "${p}/${PROJECT_NAME}" yes
	done
	;;
--user)
	remove_at "${USER_PATH}/${PROJECT_NAME}" no
	;;
--all)
	for p in "${SYSTEM_PATHS[@]}"; do
		remove_at "${p}/${PROJECT_NAME}" yes
	done
	remove_at "${USER_PATH}/${PROJECT_NAME}" no
	;;
esac

if [[ ${KEEP_CONFIG} -eq 0 ]]; then
	# Default: remove config files too (symlink-safe: never rm -rf)
	echo ">> Removing config"
	case "${MODE}" in
	--system)
		remove_config_dir "/etc/${PROJECT_NAME}" yes
		;;
	--user)
		remove_config_dir "${HOME}/.config/${PROJECT_NAME}" no
		;;
	--all)
		remove_config_dir "/etc/${PROJECT_NAME}" yes
		remove_config_dir "${HOME}/.config/${PROJECT_NAME}" no
		;;
	esac
elif [[ -f "${HOME}/.config/${PROJECT_NAME}/config.toml" ]] || [[ -d "/etc/${PROJECT_NAME}" ]]; then
	echo "   NOTE: config preserved (--keep-config)"
	echo "         user: ~/.config/${PROJECT_NAME}/config.toml"
	echo "         system: /etc/${PROJECT_NAME}/config.toml"
fi

if [[ ${removed} -eq 0 ]]; then
	echo "   (nothing found to remove)"
	exit 0
fi

echo ">> Done. Removed ${removed} artifact(s)."
