#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky (oxyzenQ)
#
# Uninstall cosmostrix: binary only. Config is PRESERVED (may contain
# user customizations). Pass --purge to also remove config.
#
# Auto-detects and removes the binary from:
#   /usr/bin/, /usr/local/bin/, ~/.local/bin/
# Sudo is used ONLY for system paths. Run WITHOUT sudo.

set -uo pipefail

PROJECT_NAME="cosmostrix"

usage() {
    cat <<EOF
Usage: $0 [--system|--user|--all] [--purge]

  (default)  Auto-detect: scan /usr/bin, /usr/local/bin, ~/.local/bin
             and remove every ${PROJECT_NAME} binary found.
  --system   Remove only from /usr/bin and /usr/local/bin (uses sudo).
  --user     Remove only from ~/.local/bin (no sudo).
  --all      Same as default.
  --purge    Also remove config files (/etc/${PROJECT_NAME},
             ~/.config/${PROJECT_NAME}).

Sudo is used only for system paths. Run WITHOUT sudo.
EOF
}

MODE="--all"
PURGE=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) MODE="--system"; shift ;;
        --user)   MODE="--user";   shift ;;
        --all)    MODE="--all";    shift ;;
        --purge)  PURGE=1;         shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

SYSTEM_PATHS=(/usr/bin /usr/local/bin)
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
        removed=$((removed+1))
    fi
}

remove_dir() {
    local target="$1"
    local need_sudo="$2"
    if [[ -d "${target}" ]]; then
        if [[ "${need_sudo}" == "yes" ]]; then
            sudo rm -rf "${target}"
        else
            rm -rf "${target}"
        fi
        echo "   removed: ${target}/"
        removed=$((removed+1))
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

if [[ ${PURGE} -eq 1 ]]; then
    echo ">> Purging config (--purge)"
    remove_dir "/etc/${PROJECT_NAME}" yes
    remove_dir "${HOME}/.config/${PROJECT_NAME}" no
elif [[ -f "${HOME}/.config/${PROJECT_NAME}/config.toml" ]]; then
    echo "   NOTE: user config preserved at ~/.config/${PROJECT_NAME}/config.toml"
    echo "         remove with: ./scripts/uninstall.sh --purge"
fi

if [[ ${removed} -eq 0 ]]; then
    echo "   (nothing found to remove)"
    exit 0
fi

echo ">> Done. Removed ${removed} artifact(s)."
