#!/bin/sh
# Builds the Glove OS ISO inside a Debian container (used by .github/workflows/iso.yml).
# Runs as root in a privileged debian:trixie container with the repository in /src, so
# the glove package is linked against the same libraries as the Debian the ISO ships.
# Leaves in pacotes/ the .deb packages and in iso/ the ISO.
set -e
cd "$(dirname "$0")/.."

export DEBIAN_FRONTEND=noninteractive
apt-get update
# Glove build dependencies (the same list as ./glove-setup-tool.sh deps), plus what
# live-build and debootstrap need on the host.
# O que o live-build e o debootstrap precisam no sistema; com GLOVE_USE_PACKAGES, os
# .deb já vêm prontos (ci/pacotes.sh) e não é preciso compilar.
if [ -n "${GLOVE_USE_PACKAGES:-}" ]; then
    apt-get install -y --no-install-recommends \
        ca-certificates curl git sudo dpkg-dev debian-archive-keyring gpgv xz-utils zstd \
        debootstrap cpio file rsync xorriso squashfs-tools mtools dosfstools
else
    apt-get install -y --no-install-recommends \
        build-essential pkg-config libudev-dev libinput-dev libseat-dev libgbm-dev libdrm-dev \
        libxkbcommon-dev libegl-dev libgles-dev fontconfig fonts-dejavu-core \
        ca-certificates curl git sudo dpkg-dev debian-archive-keyring gpgv xz-utils zstd \
        debootstrap cpio file rsync xorriso squashfs-tools mtools dosfstools
    # Rust from rustup (the Debian one may be older than the crates need).
    if ! command -v cargo >/dev/null 2>&1; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    fi
    . "$HOME/.cargo/env"
fi

# The repository belongs to another user (the runner's): let Git use it.
git config --global --add safe.directory '*'

./glove-setup-tool.sh iso
