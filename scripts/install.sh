#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky (oxyzenQ)
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses `sudo`, `install -Dm755`, `cargo`, `uname -m`, /proc/cpuinfo (for
#   CPU autodetect on Linux x86-64). macOS/BSD fall through to plain
#   `cargo build --release` (no AVX profile). Will not run on Windows
#   cmd.exe / PowerShell.
# ─────────────────────────────────────────────────────────────────────────────
#
# Install cosmostrix: binary + config.toml.
# Supports --system (system-wide) and --user (default, ~/.local).
# Run WITHOUT sudo: the script escalates via sudo ONLY for --system install steps.

set -euo pipefail

PROJECT_NAME="cosmostrix"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
    cat <<EOF
Usage: $0 [--system|--user] [--force]

  --system   Install system-wide:
               binary  → /usr/bin/${PROJECT_NAME}
               config  → /etc/${PROJECT_NAME}/config.toml
             (script invokes sudo for the install steps)

             Side effects when switching from --user to --system:
               - Stale user-local binary (~/.local/bin/${PROJECT_NAME}) is removed
               - Stale user-local systemd unit (~/.config/systemd/user/${PROJECT_NAME}.service) is removed
               - System config (/etc/${PROJECT_NAME}/config.toml) is preserved unless --force is given
               - User-local config (~/.config/${PROJECT_NAME}/config.toml) is preserved if customized

  --user     Install to user-local (default, no sudo):
               binary  → ~/.local/bin/${PROJECT_NAME}
               config  → ~/.config/${PROJECT_NAME}/config.toml
             The user-local config is NEVER overwritten unless --force is given.
             Without --force: if config exists, the new template is installed
             as config.new for manual review.
             With --force: existing config is overwritten with the new template.

  --force    Overwrite existing config.toml without creating .new backup.
             Applies to both --system and --user modes.
             Without --force: if config exists, new template is installed
             as config.new for manual review.
             With --force: existing config is overwritten directly.
             DANGEROUS: destroys customizations. Use with caution.

CPU autodetect (Linux/x86-64 only):
  The build step auto-detects the CPU microarchitecture and picks the
  optimal cargo profile:
    AVX-512 (x86-64-v4) → cargo pro-linux-v4 --locked
    AVX2    (x86-64-v3) → cargo pro-linux-v3 --locked
    baseline / non-x86  → cargo build --release --locked
  No manual profile selection needed — just run: $0

The build step ALWAYS runs as the current user (never as root).
EOF
}

MODE="--user"
FORCE=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) MODE="--system"; shift ;;
        --user)   MODE="--user";   shift ;;
        --force)  FORCE=1;        shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

# Refuse to run as root — cargo build must run as the current user.
# If run with sudo, cargo build would create root-owned files in target/,
# breaking future `cargo clean` / `cargo build` for the normal user.
# The script uses sudo internally only for the install step in --system mode.
if [[ $EUID -eq 0 ]]; then
    echo "error: do not run this script with sudo." >&2
    echo "  cargo build would run as root, corrupting target/ ownership." >&2
    echo "  Run: $0 --system" >&2
    echo "  The script will use sudo internally only for the install step." >&2
    exit 1
fi

cd "${REPO_ROOT}"

if [[ ! -f Cargo.toml ]]; then
    echo "error: Cargo.toml not found." >&2
    exit 1
fi

# ── Helper: detect + clean stale user-local install when switching to --system ──
# Removes ~/.local/bin/${PROJECT_NAME} and ~/.config/systemd/user/${PROJECT_NAME}.service
# if they exist (leftover from a previous --user install). Prints a warning
# before deleting. User-local config is NOT touched here — that's handled
# separately by preserve_or_clean_user_config.
cleanup_user_local_install() {
    local stale_paths=()
    local user_bin="${HOME}/.local/bin/${PROJECT_NAME}"
    local user_unit="${HOME}/.config/systemd/user/${PROJECT_NAME}.service"

    if [[ -f "${user_bin}" ]]; then
        stale_paths+=("${user_bin}")
    fi
    if [[ -f "${user_unit}" ]]; then
        stale_paths+=("${user_unit}")
    fi

    if [[ ${#stale_paths[@]} -eq 0 ]]; then
        return 0
    fi

    echo "   WARNING: Stale user-local install detected (from previous --user install)."
    echo "   The following will be removed to prevent PATH confusion:"
    for p in "${stale_paths[@]}"; do
        echo "     - ${p}"
    done
    echo "   (User-local config at ~/.config/${PROJECT_NAME}/config.toml is preserved if customized.)"
    for p in "${stale_paths[@]}"; do
        rm -f "${p}"
        echo "   removed: ${p}"
    done
}

# ── Helper: preserve or clean user-local config when installing --system ──
# If the user-local config matches the generated default → remove it (bloat).
# If it differs (user customized it) → preserve it (it takes precedence
# over the system config in /etc due to XDG_CONFIG_HOME precedence).
preserve_or_clean_user_config() {
    local binary="$1"
    local user_cfg="${HOME}/.config/${PROJECT_NAME}/config.toml"
    if [[ ! -f "${user_cfg}" ]]; then
        return 0
    fi

    # Strip blank lines + comments for fair comparison.
    local user_normalized default_normalized
    user_normalized=$(grep -vE '^\s*#|^\s*$' "${user_cfg}" 2>/dev/null || true)
    # Generate default config to a temp file, then compare.
    # Cannot use stdout redirect (blocked by safe-path check), so use
    # --dump-config with a temp path inside the allowed config dir.
    local tmp_default="${HOME}/.config/${PROJECT_NAME}/.install_tmp_default.toml"
    "${binary}" --dump-config "${tmp_default}" 2>/dev/null
    default_normalized=$(grep -vE '^\s*#|^\s*$' "${tmp_default}" 2>/dev/null || true)
    rm -f "${tmp_default}"

    if [[ "${user_normalized}" == "${default_normalized}" ]]; then
        echo "   user-local config matches default — removing to avoid bloat:"
        echo "     ${user_cfg}"
        rm -f "${user_cfg}"
        # Also remove the parent dir if empty (don't leave empty ~/.config/cosmostrix).
        rmdir "$(dirname "${user_cfg}")" 2>/dev/null || true
    else
        echo "   User-local config is customized — preserved at:"
        echo "     ${user_cfg}"
        echo "   (XDG_CONFIG_HOME precedence: user-local config overrides /etc config.)"
    fi
}

echo ">> [1/4] Building ${PROJECT_NAME} (autodetect CPU, locked)"

# Detect CPU microarchitecture level and pick the optimal build profile.
# x86-64-v4 (AVX-512) > x86-64-v3 (AVX2) > native release.
# Falls back to plain 'cargo build --release' on non-x86 or older CPUs.
detect_build_profile() {
    local arch
    arch="$(uname -m 2>/dev/null || echo unknown)"

    if [[ "${arch}" != "x86_64" && "${arch}" != "amd64" ]]; then
        echo "release"
        return
    fi

    # Check /proc/cpuinfo for AVX-512 (v4) then AVX2 (v3).
    local cpuinfo
    cpuinfo="$(grep -m1 '^flags' /proc/cpuinfo 2>/dev/null || true)"

    if echo "${cpuinfo}" | grep -qw avx512f; then
        echo "pro-linux-v4"
    elif echo "${cpuinfo}" | grep -qw avx2; then
        echo "pro-linux-v3"
    else
        echo "release"
    fi
}

BUILD_PROFILE="$(detect_build_profile)"
case "${BUILD_PROFILE}" in
    pro-linux-v4)
        echo "   detected: x86-64-v4 (AVX-512) — using pro-linux-v4 profile"
        cargo pro-linux-v4 --locked
        BINARY="target/x86_64-unknown-linux-gnu/pro-linux-v4/${PROJECT_NAME}"
        ;;
    pro-linux-v3)
        echo "   detected: x86-64-v3 (AVX2) — using pro-linux-v3 profile"
        cargo pro-linux-v3 --locked
        BINARY="target/x86_64-unknown-linux-gnu/pro-linux-v3/${PROJECT_NAME}"
        ;;
    *)
        echo "   detected: baseline x86-64 or non-x86 — using release profile"
        cargo build --release --locked
        BINARY="target/release/${PROJECT_NAME}"
        ;;
esac

if [[ ! -f "${BINARY}" ]]; then
    echo "error: build produced no binary at ${BINARY}" >&2
    exit 1
fi

echo ">> [2/4] Installing binary (${MODE})"
case "${MODE}" in
    --system)
        # Clean up stale user-local install before installing system-wide.
        # This prevents PATH confusion where ~/.local/bin/cosmostrix shadows
        # /usr/bin/cosmostrix after a --system install.
        cleanup_user_local_install
        sudo install -Dm755 "${BINARY}" "/usr/bin/${PROJECT_NAME}"
        echo "   installed: /usr/bin/${PROJECT_NAME}"
        ;;
    --user)
        user_bin="${HOME}/.local/bin"
        mkdir -p "${user_bin}"
        install -Dm755 "${BINARY}" "${user_bin}/${PROJECT_NAME}"
        echo "   installed: ${user_bin}/${PROJECT_NAME}"
        ;;
esac

# Build --force flag for --dump-config pass-through.
# The Rust binary's --dump-config refuses to overwrite existing files
# unless --force is also passed. When the script's FORCE=1, we embed
# --force in the --dump-config call so the binary's guard is bypassed.
FORCE_ARG=()
if [[ ${FORCE} -eq 1 ]]; then
    FORCE_ARG=("--force")
fi

echo ">> [3/4] Installing config.toml (${MODE})"
case "${MODE}" in
    --system)
        sudo mkdir -p "/etc/${PROJECT_NAME}"
        config_path="/etc/${PROJECT_NAME}/config.toml"
        if sudo test -f "${config_path}"; then
            if [[ ${FORCE} -eq 1 ]]; then
                # --force: overwrite existing system config directly.
                # Pass --force to --dump-config so the binary skips its
                # existing-file guard too.
                sudo "${BINARY}" --dump-config "${config_path}" "${FORCE_ARG[@]}" 2>/dev/null
                echo "   overwritten: ${config_path} (--force)"
            else
                # Existing config: write new template to .new for manual merge.
                sudo "${BINARY}" --dump-config "${config_path}.new" 2>/dev/null
                echo "   existing config preserved: ${config_path}"
                echo "   new template installed at: ${config_path}.new (review and merge manually)"
                echo "   use --force to overwrite without backup"
            fi
        else
            # No existing config: write template directly.
            sudo "${BINARY}" --dump-config "${config_path}" 2>/dev/null
            echo "   installed: ${config_path}"
        fi
        # Preserve user-local config if customized; clean up if default.
        preserve_or_clean_user_config "${BINARY}"
        ;;
    --user)
        user_cfg_dir="${HOME}/.config/${PROJECT_NAME}"
        user_cfg="${user_cfg_dir}/config.toml"
        mkdir -p "${user_cfg_dir}"
        if [[ -f "${user_cfg}" ]]; then
            if [[ ${FORCE} -eq 1 ]]; then
                # --force: overwrite existing config directly.
                # Pass --force to --dump-config so the binary skips its
                # existing-file guard too.
                "${BINARY}" --dump-config "${user_cfg}" "${FORCE_ARG[@]}" 2>/dev/null
                echo "   overwritten: ${user_cfg} (--force)"
            else
                # Existing config: write new template to .new for manual merge.
                # Use --dump-config <path> directly (not stdout redirect).
                "${BINARY}" --dump-config "${user_cfg}.new" 2>/dev/null
                echo "   existing config preserved: ${user_cfg}"
                echo "   new template installed at: ${user_cfg}.new (review and merge manually)"
                echo "   use --force to overwrite without backup"
            fi
        else
            # No existing config: write template directly.
            "${BINARY}" --dump-config "${user_cfg}" 2>/dev/null
            echo "   installed: ${user_cfg}"
        fi
        ;;
esac

echo ">> [4/4] Post-install verification"
case "${MODE}" in
    --system)
        echo "  - Verify: ${PROJECT_NAME} --version"
        echo "  - System config: /etc/${PROJECT_NAME}/config.toml"
        ;;
    --user)
        echo "  - Ensure ~/.local/bin is on your PATH"
        echo "  - Verify: ${PROJECT_NAME} --version"
        ;;
esac
echo "  - Validate config: ${PROJECT_NAME} --testconf"
echo "  - Uninstall: ./scripts/uninstall.sh ${MODE}"
echo
echo ">> Done."