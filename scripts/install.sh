#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

# SPDX-License-Identifier: GPL-3.0-only
set -euo pipefail

PROJECT_NAME="cosmostrix"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ── Usage ───────────────────────────────────────────────────────────────────
usage() {
    cat <<EOF
Usage: $0 [--system | --user] [--no-build]

Install cosmostrix binary + default config.

Modes:
  --user     (default) Install to ~/.local/bin/, config to ~/.config/cosmostrix/config
  --system   Install to /usr/local/bin/, config to /etc/cosmostrix/config (requires root)

Options:
  --no-build Skip cargo build (assume target/release/cosmostrix already exists)
  -h, --help Show this help

The config file is NEVER overwritten. If it already exists, a timestamped
backup is created as config.bak.<epoch> and the new template is installed
as config.new for manual review.

Environment overrides:
  PREFIX     Custom install prefix (e.g. PREFIX=/opt/cosmostrix)
  DESTDIR    Staging dir for package builds (e.g. DESTDIR=/tmp/pkg)
EOF
}

# ── Parse args ──────────────────────────────────────────────────────────────
MODE="--user"
NO_BUILD=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system)  MODE="--system"; shift ;;
        --user)    MODE="--user"; shift ;;
        --no-build) NO_BUILD=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage >&2; exit 2 ;;
    esac
done

# ── Determine paths based on mode ───────────────────────────────────────────
if [[ "$MODE" == "--system" ]]; then
    PREFIX="${PREFIX:-/usr/local}"
    CONFIG_DIR="/etc/cosmostrix"
    CONFIG_PATH="${CONFIG_DIR}/config"
    if [[ $EUID -ne 0 ]]; then
        echo "error: --system requires root (use sudo or run as root)" >&2
        exit 1
    fi
else
    PREFIX="${PREFIX:-${HOME}/.local}"
    CONFIG_DIR="${HOME}/.config/cosmostrix"
    CONFIG_PATH="${CONFIG_DIR}/config"
fi

BINDIR="${DESTDIR:-}${PREFIX}/bin"
CONFIG_DIR_STAGED="${DESTDIR:-}${CONFIG_DIR}"
CONFIG_PATH_STAGED="${CONFIG_DIR_STAGED}/config"

cd "${REPO_ROOT}"

# ── Build ───────────────────────────────────────────────────────────────────
if [[ $NO_BUILD -eq 0 ]]; then
    echo "==> Building cosmostrix (cargo build --release --locked)..."
    cargo build --release --locked
fi

if [[ ! -f "target/release/${PROJECT_NAME}" ]]; then
    echo "error: target/release/${PROJECT_NAME} not found. Run without --no-build or build first." >&2
    exit 1
fi

# ── Install binary ──────────────────────────────────────────────────────────
echo "==> Installing binary to ${BINDIR}/${PROJECT_NAME}..."
mkdir -p "${BINDIR}"
install -m 755 "target/release/${PROJECT_NAME}" "${BINDIR}/${PROJECT_NAME}"

# ── Install config (with auto-backup, never overwrite) ─────────────────────
echo "==> Installing config to ${CONFIG_PATH_STAGED}..."
mkdir -p "${CONFIG_DIR_STAGED}"

CONFIG_TEMPLATE="${REPO_ROOT}/config/config.toml"
if [[ ! -f "${CONFIG_TEMPLATE}" ]]; then
    echo "warning: config template not found at ${CONFIG_TEMPLATE}, skipping config install" >&2
else
    if [[ -f "${CONFIG_PATH_STAGED}" ]]; then
        # Config already exists — backup + install as .new for review
        BACKUP="${CONFIG_PATH_STAGED}.bak.$(date +%s)"
        cp -p "${CONFIG_PATH_STAGED}" "${BACKUP}"
        install -m 644 "${CONFIG_TEMPLATE}" "${CONFIG_PATH_STAGED}.new"
        echo "    existing config preserved at: ${CONFIG_PATH_STAGED}"
        echo "    backup created at:            ${BACKUP}"
        echo "    new template installed at:    ${CONFIG_PATH_STAGED}.new (review and merge manually)"
    else
        install -m 644 "${CONFIG_TEMPLATE}" "${CONFIG_PATH_STAGED}"
        echo "    config installed at: ${CONFIG_PATH_STAGED}"
    fi
fi

# ── Summary ────────────────────────────────────────────────────────────────
echo ""
echo "==> Installation complete!"
echo "    Binary:  ${BINDIR}/${PROJECT_NAME}"
echo "    Config:  ${CONFIG_PATH}"
echo ""
if [[ "$MODE" == "--system" ]]; then
    echo "    Make sure ${BINDIR} is in your PATH (usually is for /usr/local/bin)."
    echo "    Edit ${CONFIG_PATH} to customize, then run: cosmostrix --testconf"
else
    echo "    Make sure ${BINDIR} is in your PATH."
    echo "    Add this to your shell rc (~/.bashrc or ~/.zshrc):"
    echo "      export PATH=\"${BINDIR}:\$PATH\""
    echo "    Edit ${CONFIG_PATH} to customize, then run: cosmostrix --testconf"
fi
echo ""
echo "    Verify installation: ${BINDIR}/${PROJECT_NAME} --version"
