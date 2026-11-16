#!/bin/sh
# Compila o Glove e gera os pacotes glove e glove-os num contêiner Debian trixie (a
# base da ISO), para o repositório apt e a ISO (.github/workflows/iso.yml).
set -e
cd "$(dirname "$0")/.."
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
    build-essential pkg-config libudev-dev libinput-dev libseat-dev libgbm-dev libdrm-dev \
    libxkbcommon-dev libegl-dev libgles-dev fontconfig fonts-dejavu-core \
    ca-certificates curl git dpkg-dev zip gtk-update-icon-cache
# Rust pelo rustup (o do Debian é mais velho que o mínimo das dependências).
if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
. "$HOME/.cargo/env"
git config --global --add safe.directory '*'
./glove-setup-tool.sh deb
./glove-setup-tool.sh glove-os
./glove-setup-tool.sh mide-icons
ls -l pacotes/
