#!/bin/sh
# Compila o Glove e gera o pacote .rpm num contêiner Fedora
# (.github/workflows/pacotes-distros.yml). O pacote serve para o Fedora em que foi
# compilado e para os mais novos.
set -e
cd "$(dirname "$0")/.."
. ci/comum.sh
dnf install -y --setopt=install_weak_deps=False \
    gcc pkgconf-pkg-config systemd-devel libinput-devel libseat-devel mesa-libgbm-devel \
    libdrm-devel libxkbcommon-devel mesa-libEGL-devel fontconfig dejavu-sans-fonts dejavu-sans-mono-fonts \
    rpm-build curl ca-certificates findutils tar gzip which
instalar_rust
./glove-setup-tool.sh rpm
ls pacotes/glove-*.rpm
# O dnf instala também as dependências do pacote (greetd, Mesa…): confere os nomes.
dnf install -y --setopt=install_weak_deps=False ./pacotes/glove-*.rpm
conferir
# A Jangada vai junto para o repositório dnf (e é conferida: instala e acha as bibliotecas).
baixar_jangada 'jangada-{v}-1.x86_64.rpm'
dnf install -y --setopt=install_weak_deps=False ./pacotes/jangada-*.rpm
falta=$(ldd /usr/lib/jangada/jangada | grep "not found" || true)
[ -z "$falta" ] || { echo "ERRO: a Jangada não acha:"; echo "$falta"; exit 1; }
