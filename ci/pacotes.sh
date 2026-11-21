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
# O navegador (a Jangada, repositório próprio): o pacote pronto da release dela. Sem ele
# (a release ainda não saiu), compila do código daquela versão.
jv=$(sed -n 's/^jangada_version=//p' glove-setup-tool.sh | head -1)
if ! curl -fsSI --max-time 60 "https://github.com/fabioaacarneiro/jangada/releases/download/v$jv/jangada_${jv}_amd64.deb" >/dev/null 2>&1; then
    apt-get install -y --no-install-recommends cmake ninja-build binutils xz-utils libwayland-dev \
        libnss3 libnspr4 libatk1.0-0t64 libatk-bridge2.0-0t64 libcups2t64 libxcomposite1 libxdamage1 \
        libxrandr2 libxext6 libxfixes3 libx11-6 libxcb1 libgbm1 libdrm2 libpango-1.0-0 libcairo2 \
        libasound2t64 libdbus-1-3 libexpat1 libudev1 libglib2.0-0t64
fi
./glove-setup-tool.sh jangada
ls -l pacotes/
