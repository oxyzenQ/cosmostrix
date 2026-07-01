#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only

set -euo pipefail

PROJECT_NAME="cosmostrix"
PREFIX="${PREFIX:-${HOME}/.local}"
BINDIR="${DESTDIR:-}${PREFIX}/bin"

rm -f "${BINDIR}/${PROJECT_NAME}"
echo "${PROJECT_NAME} removed from ${BINDIR}/${PROJECT_NAME}"
