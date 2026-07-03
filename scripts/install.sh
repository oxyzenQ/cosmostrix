#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky (oxyzenQ)
#
# Install cosmostrix: binary + config.toml (with auto-backup, never overwrite).
# Supports --system (system-wide) and --user (default, ~/.local).
# Run WITHOUT sudo: the script escalates via sudo ONLY for --system install steps.

set -euo pipefail

PROJECT_NAME="cosmostrix"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG_SRC="${REPO_ROOT}/config/config.toml"

usage() {
    cat <<EOF
Usage: $0 [--system|--user]

  --system   Install system-wide:
               binary  → /usr/local/bin/${PROJECT_NAME}
               config  → /etc/${PROJECT_NAME}/config.toml
             (script invokes sudo for the install steps)
  --user     Install to user-local (default, no sudo):
               binary  → ~/.local/bin/${PROJECT_NAME}
               config  → ~/.config/${PROJECT_NAME}/config.toml

The config file is NEVER overwritten. If it already exists, a timestamped
backup is created as config.bak.<epoch> and the new template is installed
as config.new for manual review.

The build step (cargo build --release --locked) ALWAYS runs as the current user.
EOF
}

MODE="--user"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) MODE="--system"; shift ;;
        --user)   MODE="--user";   shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

cd "${REPO_ROOT}"

if [[ ! -f Cargo.toml ]]; then
    echo "error: Cargo.toml not found." >&2
    exit 1
fi

if [[ ! -f "${CONFIG_SRC}" ]]; then
    echo "error: config template not found: ${CONFIG_SRC}" >&2
    exit 1
fi

echo ">> [1/3] Building ${PROJECT_NAME} (release, locked)"
cargo build --release --locked

BINARY="target/release/${PROJECT_NAME}"
if [[ ! -f "${BINARY}" ]]; then
    echo "error: build produced no binary at ${BINARY}" >&2
    exit 1
fi

echo ">> [2/3] Installing binary (${MODE})"
case "${MODE}" in
    --system)
        sudo install -Dm755 "${BINARY}" "/usr/local/bin/${PROJECT_NAME}"
        echo "   installed: /usr/local/bin/${PROJECT_NAME}"
        ;;
    --user)
        user_bin="${HOME}/.local/bin"
        mkdir -p "${user_bin}"
        install -Dm755 "${BINARY}" "${user_bin}/${PROJECT_NAME}"
        echo "   installed: ${user_bin}/${PROJECT_NAME}"
        ;;
esac

echo ">> [3/3] Installing config.toml (${MODE})"
case "${MODE}" in
    --system)
        sudo mkdir -p "/etc/${PROJECT_NAME}"
        config_path="/etc/${PROJECT_NAME}/config.toml"
        if sudo test -f "${config_path}"; then
            backup="${config_path}.bak.$(date +%s)"
            sudo cp -p "${config_path}" "${backup}"
            sudo install -m 644 "${CONFIG_SRC}" "${config_path}.new"
            echo "   existing config preserved: ${config_path}"
            echo "   backup created at:            ${backup}"
            echo "   new template installed at:    ${config_path}.new (review and merge manually)"
        else
            sudo install -m 644 "${CONFIG_SRC}" "${config_path}"
            echo "   installed: ${config_path}"
        fi
        ;;
    --user)
        user_cfg_dir="${HOME}/.config/${PROJECT_NAME}"
        user_cfg="${user_cfg_dir}/config.toml"
        mkdir -p "${user_cfg_dir}"
        if [[ -f "${user_cfg}" ]]; then
            backup="${user_cfg}.bak.$(date +%s)"
            cp -p "${user_cfg}" "${backup}"
            install -m 644 "${CONFIG_SRC}" "${user_cfg}.new"
            echo "   existing config preserved: ${user_cfg}"
            echo "   backup created at:            ${backup}"
            echo "   new template installed at:    ${user_cfg}.new (review and merge manually)"
        else
            install -m 644 "${CONFIG_SRC}" "${user_cfg}"
            echo "   installed: ${user_cfg}"
        fi
        ;;
esac

echo
echo ">> Done."
echo
echo "Next steps:"
case "${MODE}" in
    --system) echo "  - Verify: ${PROJECT_NAME} --version" ;;
    --user)
        echo "  - Ensure ~/.local/bin is on your PATH"
        echo "  - Verify: ${PROJECT_NAME} --version"
        ;;
esac
echo "  - Validate config: ${PROJECT_NAME} --testconf"
echo "  - Uninstall: ./scripts/uninstall.sh"
