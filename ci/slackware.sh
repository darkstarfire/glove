#!/bin/sh
# Compila o Glove e gera o pacote do Slackware (.tgz) num contêiner Slackware 15.0
# (.github/workflows/pacotes-distros.yml). Compilado no 15.0, o pacote serve para o
# 15.0 e para o -current.
#
# O contêiner é a instalação mínima, e o Slackware não resolve dependências: a lista
# abaixo é o que o compilador e as bibliotecas do Glove precisam (conferida com o ldd).
set -e
cd "$(dirname "$0")/.."
. ci/comum.sh
sp() { slackpkg -batch=on -default_answer=y "$@"; }
yes | sp update gpg >/dev/null
sp update >/dev/null
# - Compilar: gcc, binutils (o ar precisa do flex), make (precisa do guile e do gc).
# - Bibliotecas do Glove e o que elas puxam: libinput, Mesa (gbm, EGL), xkbcommon, udev.
# - A fonte do Glove (o gerador dos ícones a usa).
# - Para a libseat: meson, ninja e python3. Para baixar: certificados (o perl os instala).
sp install gcc binutils flex kernel-headers pkg-config libmpc mpfr gmp isl zlib make guile gc \
    libinput libevdev mtdev libwacom eudev libgudev glib2 pcre libffi \
    mesa libglvnd libdrm libpciaccess wayland expat libxkbcommon dejavu-fonts-ttf \
    meson ninja python3 ca-certificates openssl perl >/dev/null
update-ca-certificates --fresh >/dev/null 2>&1
command -v gcc >/dev/null && command -v ar >/dev/null && command -v meson >/dev/null || { echo "ERRO: o slackpkg não instalou o compilador"; exit 1; }

# A libseat não vem no Slackware 15.0 (é o seatd, do SlackBuilds.org, que o pacote do
# Glove pede). Aqui só para compilar: a do computador de quem instala é a do seatd dele.
seatd=0.9.1
wget -q -O /tmp/seatd.tar.gz "https://git.sr.ht/~kennylevinsen/seatd/archive/$seatd.tar.gz"
echo "819979c922a0be258aed133d93920bce6a3d3565a60588d6d372ce9db2712cd3  /tmp/seatd.tar.gz" | sha256sum -c -
tar xzf /tmp/seatd.tar.gz -C /tmp
(cd "/tmp/seatd-$seatd" && meson setup build --prefix=/usr --libdir=lib64 --buildtype=release \
    -Dserver=disabled -Dexamples=disabled -Dman-pages=disabled -Dlibseat-logind=disabled >/dev/null && ninja -C build install >/dev/null)
pkg-config --modversion libseat

instalar_rust
./glove-setup-tool.sh slackware
ls pacotes/glove-*.tgz
installpkg pacotes/glove-*.tgz
conferir
